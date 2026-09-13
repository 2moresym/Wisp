use std::f32::consts::PI;

use crate::brain::VisualInput;

#[derive(Clone, Copy, Debug)]
pub struct Fly {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub rotation_y: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Food {
    pub position: [f32; 3],
    pub radius: f32,
}

impl Default for Fly {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            rotation_y: 0.0,
        }
    }
}

impl Default for Food {
    fn default() -> Self {
        Self {
            position: [4.0, 0.0, 0.0],
            radius: 0.45,
        }
    }
}

impl Fly {
    pub fn visual_input(&self, food: &Food) -> VisualInput {
        let dx = food.position[0] - self.position[0];
        let dz = food.position[2] - self.position[2];
        let distance = (dx * dx + dz * dz).sqrt();
        let world_angle = dz.atan2(dx);
        let relative = normalize_angle(world_angle - self.rotation_y);

        VisualInput {
            angle: relative,
            distance,
        }
    }

    pub fn apply_motor(&mut self, turn: f32, forward: f32, dt: f32) {
        // Motor output is intentionally direct: turn changes yaw and forward
        // produces acceleration along the fly's local forward vector.
        self.rotation_y += turn * 3.0 * dt;
        let speed = 2.0 * forward;
        let forward_x = self.rotation_y.cos();
        let forward_z = self.rotation_y.sin();
        self.velocity[0] += forward_x * speed * dt;
        self.velocity[2] += forward_z * speed * dt;
    }

    pub fn integrate(&mut self, dt: f32) {
        self.position[0] += self.velocity[0] * dt;
        self.position[2] += self.velocity[2] * dt;
        self.velocity[0] *= 0.92_f32.powf(dt * 60.0);
        self.velocity[2] *= 0.92_f32.powf(dt * 60.0);

        // Cheap square arena bounds.
        for axis in [0usize, 2usize] {
            if self.position[axis] > 8.0 {
                self.position[axis] = 8.0;
                self.velocity[axis] *= -0.4;
            } else if self.position[axis] < -8.0 {
                self.position[axis] = -8.0;
                self.velocity[axis] *= -0.4;
            }
        }
    }

    pub fn collides_with(&self, food: &Food) -> bool {
        let dx = self.position[0] - food.position[0];
        let dz = self.position[2] - food.position[2];
        dx * dx + dz * dz <= (food.radius + 0.35).powi(2)
    }

    pub fn reset_near_origin(&mut self) {
        self.position = [0.0, 0.0, 0.0];
        self.velocity = [0.0, 0.0, 0.0];
        self.rotation_y = PI;
    }
}

fn normalize_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}
