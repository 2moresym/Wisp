use std::f32::consts::PI;

use crate::brain::VisualInput;

pub const ARENA_HALF_SIZE: f32 = 8.0;
pub const FLY_RADIUS: f32 = 0.28;

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
            position: [0.0, 0.15, 0.0],
            velocity: [0.0; 3],
            rotation_y: 0.0,
        }
    }
}

impl Default for Food {
    fn default() -> Self {
        Self {
            position: [5.0, 0.35, 2.0],
            radius: 0.45,
        }
    }
}

impl Fly {
    pub fn forward(&self) -> [f32; 3] {
        [self.rotation_y.cos(), 0.0, self.rotation_y.sin()]
    }

    pub fn visual_input(&self, food: &Food) -> VisualInput {
        let dx = food.position[0] - self.position[0];
        let dz = food.position[2] - self.position[2];
        let distance = (dx * dx + dz * dz).sqrt();
        let world_angle = dz.atan2(dx);
        let angle = normalize_angle(world_angle - self.rotation_y);
        let f = self.forward();
        let lateral = (dx * (-f[2]) + dz * f[0]) / distance.max(0.001);
        let line_x = dx / distance.max(0.001);
        let line_z = dz / distance.max(0.001);
        let approach_velocity = self.velocity[0] * line_x + self.velocity[2] * line_z;
        VisualInput {
            angle,
            distance,
            lateral,
            approach_velocity,
        }
    }

    pub fn apply_motor(&mut self, turn: f32, forward: f32, dt: f32) {
        self.rotation_y = normalize_angle(self.rotation_y + turn.clamp(-1.0, 1.0) * 3.5 * dt);
        let f = self.forward();
        let thrust = forward.clamp(0.0, 1.0) * 4.0;
        self.velocity[0] += f[0] * thrust * dt;
        self.velocity[2] += f[2] * thrust * dt;
    }

    pub fn integrate(&mut self, dt: f32) {
        self.position[0] += self.velocity[0] * dt;
        self.position[2] += self.velocity[2] * dt;
        let drag = (-3.5 * dt).exp();
        self.velocity[0] *= drag;
        self.velocity[2] *= drag;
        let speed_sq = self.velocity[0] * self.velocity[0] + self.velocity[2] * self.velocity[2];
        let max_speed = 4.0;
        if speed_sq > max_speed * max_speed {
            let scale = max_speed / speed_sq.sqrt();
            self.velocity[0] *= scale;
            self.velocity[2] *= scale;
        }
        for axis in [0usize, 2usize] {
            let limit = ARENA_HALF_SIZE - FLY_RADIUS;
            if self.position[axis] > limit {
                self.position[axis] = limit;
                self.velocity[axis] *= -0.45;
            } else if self.position[axis] < -limit {
                self.position[axis] = -limit;
                self.velocity[axis] *= -0.45;
            }
        }
        self.position[1] = 0.15;
    }

    pub fn collides_with(&self, food: &Food) -> bool {
        let dx = self.position[0] - food.position[0];
        let dz = self.position[2] - food.position[2];
        let radius = food.radius + FLY_RADIUS;
        dx * dx + dz * dz <= radius * radius
    }

    pub fn reset_near_origin(&mut self) {
        *self = Self::default();
    }
}

pub fn normalize_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_radius_works() {
        let fly = Fly::default();
        let mut food = Food::default();
        food.position = [0.5, 0.35, 0.0];
        assert!(fly.collides_with(&food));
        food.position = [3.0, 0.35, 0.0];
        assert!(!fly.collides_with(&food));
    }

    #[test]
    fn visual_input_reports_food_ahead() {
        let fly = Fly::default();
        let mut food = Food::default();
        food.position = [4.0, 0.35, 0.0];
        let visual = fly.visual_input(&food);
        assert!(visual.angle.abs() < 0.01);
        assert!(visual.distance > 3.9);
        assert!(visual.approach_velocity.abs() < f32::EPSILON);
    }

    #[test]
    fn visual_input_reports_approach_velocity() {
        let mut fly = Fly::default();
        let mut food = Food::default();
        food.position = [4.0, 0.35, 0.0];
        fly.velocity[0] = 1.0;
        assert!(fly.visual_input(&food).approach_velocity > 0.9);
    }

    #[test]
    fn motor_moves_forward() {
        let mut fly = Fly::default();
        fly.apply_motor(0.0, 1.0, 0.1);
        fly.integrate(0.1);
        assert!(fly.position[0] > 0.0);
    }

    #[test]
    fn angle_normalization_stays_in_range() {
        assert!((-PI..=PI).contains(&normalize_angle(9.0)));
        assert!((-PI..=PI).contains(&normalize_angle(-9.0)));
    }
}
