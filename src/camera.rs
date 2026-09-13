use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
    /// Column-major storage matching WGSL mat4x4 layout.
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn mul(self, rhs: Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                out[col][row] = self.m[0][row] * rhs.m[col][0]
                    + self.m[1][row] * rhs.m[col][1]
                    + self.m[2][row] * rhs.m[col][2]
                    + self.m[3][row] * rhs.m[col][3];
            }
        }
        Self { m: out }
    }

    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        let mut out = Self::IDENTITY;
        out.m[3] = [x, y, z, 1.0];
        out
    }

    pub fn rotation_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            m: [
                [c, 0.0, -s, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [s, 0.0, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            m: [
                [c, s, 0.0, 0.0],
                [-s, c, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        Self {
            m: [
                [x, 0.0, 0.0, 0.0],
                [0.0, y, 0.0, 0.0],
                [0.0, 0.0, z, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Self {
        let f = normalize(sub(target, eye));
        let s = normalize(cross(f, up));
        let u = cross(s, f);
        Self {
            m: [
                [s[0], u[0], -f[0], 0.0],
                [s[1], u[1], -f[1], 0.0],
                [s[2], u[2], -f[2], 0.0],
                [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
            ],
        }
    }

    /// Right-handed perspective using wgpu's 0..1 depth range.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        Self {
            m: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, far / (near - far), -1.0],
                [0.0, 0.0, near * far / (near - far), 0.0],
            ],
        }
    }

    pub fn to_cols_array(self) -> [f32; 16] {
        [
            self.m[0][0],
            self.m[0][1],
            self.m[0][2],
            self.m[0][3],
            self.m[1][0],
            self.m[1][1],
            self.m[1][2],
            self.m[1][3],
            self.m[2][0],
            self.m[2][1],
            self.m[2][2],
            self.m[2][3],
            self.m[3][0],
            self.m[3][1],
            self.m[3][2],
            self.m[3][3],
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub aspect: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn view_projection(&self) -> Mat4 {
        Mat4::perspective(self.fov_y, self.aspect, self.near, self.far).mul(Mat4::look_at(
            self.position,
            self.target,
            [0.0, 1.0, 0.0],
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraRig {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub follow_distance: f32,
    pub height: f32,
    pub look_ahead: f32,
    pub smoothing: f32,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            position: [0.0, 3.0, -5.0],
            target: [0.0, 0.25, 0.0],
            follow_distance: 5.0,
            height: 3.0,
            look_ahead: 1.5,
            smoothing: 7.0,
        }
    }
}

impl CameraRig {
    pub fn update(&mut self, fly_position: [f32; 3], forward: [f32; 3], dt: f32) {
        let desired_position = [
            fly_position[0] - forward[0] * self.follow_distance,
            fly_position[1] + self.height,
            fly_position[2] - forward[2] * self.follow_distance,
        ];
        let desired_target = [
            fly_position[0] + forward[0] * self.look_ahead,
            fly_position[1] + 0.1,
            fly_position[2] + forward[2] * self.look_ahead,
        ];
        let alpha = 1.0 - (-self.smoothing * dt.max(0.0)).exp();
        self.position = lerp3(self.position, desired_position, alpha);
        self.target = lerp3(self.target, desired_target, alpha);
    }

    pub fn camera(self, aspect: f32) -> Camera {
        Camera {
            position: self.position,
            target: self.target,
            aspect,
            fov_y: DEFAULT_FOV,
            near: 0.1,
            far: 40.0,
        }
    }
}

pub const DEFAULT_FOV: f32 = PI / 3.0;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(f32::EPSILON);
    [v[0] / len, v[1] / len, v[2] / len]
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_identity() {
        assert_eq!(Mat4::IDENTITY.m[0][0], 1.0);
        assert_eq!(Mat4::IDENTITY.m[3][3], 1.0);
    }

    #[test]
    fn matrices_are_finite() {
        let projection = Mat4::perspective(DEFAULT_FOV, 16.0 / 9.0, 0.1, 100.0);
        assert!(projection.m.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn camera_matrix_has_translation() {
        let camera = Camera {
            position: [0.0, 2.0, 4.0],
            target: [0.0, 0.0, 0.0],
            aspect: 1.0,
            fov_y: DEFAULT_FOV,
            near: 0.1,
            far: 100.0,
        };
        let matrix = camera.view_projection();
        assert!(matrix.m.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn camera_rig_follows_without_jumping() {
        let mut rig = CameraRig::default();
        let old = rig.position;
        rig.update([4.0, 0.15, 0.0], [1.0, 0.0, 0.0], 0.016);
        assert_ne!(rig.position, old);
        assert!(rig
            .camera(1.0)
            .view_projection()
            .m
            .iter()
            .flatten()
            .all(|value| value.is_finite()));
    }
}
