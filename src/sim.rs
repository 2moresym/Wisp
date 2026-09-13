use crate::brain::{Brain, BrainConfig, BrainSnapshot};
use crate::physics::{Fly, Food, ARENA_HALF_SIZE};

pub const NEURAL_DT: f32 = 0.002;
const REWARD_COOLDOWN: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default)]
pub struct SimulationStats {
    pub steps: u64,
    pub rewards: u64,
    pub last_turn: f32,
    pub last_forward: f32,
    pub min_food_distance: f32,
    pub total_distance: f32,
}

#[derive(Debug)]
pub struct Simulation {
    pub brain: Brain,
    pub fly: Fly,
    pub food: Food,
    reward_cooldown: f32,
    rng_state: u32,
    pub stats: SimulationStats,
}

impl Simulation {
    pub fn new() -> Self {
        Self {
            brain: Brain::new(BrainConfig::default()),
            fly: Fly::default(),
            food: Food::default(),
            reward_cooldown: 0.0,
            rng_state: 0x1357_9BDF,
            stats: SimulationStats {
                min_food_distance: f32::INFINITY,
                ..Default::default()
            },
        }
    }

    pub fn step(&mut self) -> BrainSnapshot {
        self.reward_cooldown = (self.reward_cooldown - NEURAL_DT).max(0.0);
        let visual = self.fly.visual_input(&self.food);
        self.stats.min_food_distance = self.stats.min_food_distance.min(visual.distance);
        let snapshot = self.brain.step(visual, NEURAL_DT);
        let old_position = self.fly.position;
        self.fly
            .apply_motor(snapshot.turn, snapshot.forward, NEURAL_DT);
        self.fly.integrate(NEURAL_DT);
        let dx = self.fly.position[0] - old_position[0];
        let dz = self.fly.position[2] - old_position[2];
        self.stats.total_distance += (dx * dx + dz * dz).sqrt();

        if self.reward_cooldown == 0.0 && self.fly.collides_with(&self.food) {
            self.brain.inject_dopamine(1.0);
            self.stats.rewards += 1;
            self.respawn_food();
            self.reward_cooldown = REWARD_COOLDOWN;
        }
        self.stats.steps += 1;
        self.stats.last_turn = snapshot.turn;
        self.stats.last_forward = snapshot.forward;
        snapshot
    }

    pub fn steps(&mut self, count: usize) {
        for _ in 0..count {
            self.step();
        }
    }

    pub fn reset_episode(&mut self) {
        self.fly.reset_near_origin();
        self.reward_cooldown = 0.0;
        self.stats.min_food_distance = f32::INFINITY;
        self.stats.total_distance = 0.0;
    }

    pub fn force_food_at_fly(&mut self) {
        self.food.position = self.fly.position;
    }

    pub fn set_food_ahead(&mut self, distance: f32) {
        let f = self.fly.forward();
        self.food.position = [
            self.fly.position[0] + f[0] * distance,
            0.35,
            self.fly.position[2] + f[2] * distance,
        ];
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        x
    }

    fn respawn_food(&mut self) {
        let span = (ARENA_HALF_SIZE - 1.0) * 2.0;
        let x = (self.next_u32() as f32 / u32::MAX as f32) * span - (ARENA_HALF_SIZE - 1.0);
        let z = (self.next_u32() as f32 / u32::MAX as f32) * span - (ARENA_HALF_SIZE - 1.0);
        self.food.position = [x, 0.35, z];
        if self.fly.collides_with(&self.food) {
            self.food.position[0] = (self.food.position[0] + 1.5).min(ARENA_HALF_SIZE - 1.0);
        }
    }
}

impl Default for Simulation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let mut a = Simulation::default();
        let mut b = Simulation::default();
        for _ in 0..2000 {
            a.step();
            b.step();
        }
        assert_eq!(a.stats.steps, b.stats.steps);
        assert_eq!(a.stats.rewards, b.stats.rewards);
        assert_eq!(a.brain.weight_checksum(), b.brain.weight_checksum());
        assert_eq!(a.fly.position, b.fly.position);
        assert_eq!(a.fly.rotation_y, b.fly.rotation_y);
    }

    #[test]
    fn collision_rewards_and_respawns() {
        let mut simulation = Simulation::default();
        simulation.force_food_at_fly();
        simulation.step();
        assert_eq!(simulation.stats.rewards, 1);
        assert!(!simulation.fly.collides_with(&simulation.food));
    }

    #[test]
    fn episode_reset_is_deterministic() {
        let mut simulation = Simulation::default();
        simulation.steps(100);
        simulation.reset_episode();
        assert_eq!(simulation.fly, Fly::default());
        assert_eq!(simulation.stats.total_distance, 0.0);
        assert!(simulation.stats.min_food_distance.is_infinite());
    }
}
