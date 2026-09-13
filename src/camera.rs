use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
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
        for row in 0..4 {
            for col in 0..4 {
                out[row][col] = self.m[row][0] * rhs.m[0][col]
                    + self.m[row][1] * rhs.m[1][col]
                    + self.m[row][2] * rhs.m[2][col]
                    + self.m[row][3] * rhs.m[3][col];
            }
        }
        Self { m: out }
    }

    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        let mut out = Self::IDENTITY;
        out.m[3][0] = x;
        out.m[3][1] = y;
        out.m[3][2] = z;
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

    // Right-handed perspective with wgpu's 0..1 depth range.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        Self {
            m: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, far / (near - far), -1.0],
                [0.0, 0.0, (near * far) / (near - far), 0.0],
            ],
        }
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
        Mat4::perspective(self.fov_y, self.aspect, self.near, self.far)
            .mul(Mat4::look_at(self.position, self.target, [0.0, 1.0, 0.0]))
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(f32::EPSILON);
    [v[0] / len, v[1] / len, v[2] / len]
}

pub const DEFAULT_FOV: f32 = PI / 3.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_identity() {
        assert_eq!(Mat4::IDENTITY.m[0][0], 1.0);
        assert_eq!(Mat4::IDENTITY.m[3][3], 1.0);
    }

    #[test]
    fn perspective_is_finite() {
        let p = Mat4::perspective(DEFAULT_FOV, 16.0 / 9.0, 0.1, 100.0);
        assert!(p.m.iter().flatten().all(|v| v.is_finite()));
    }
}
