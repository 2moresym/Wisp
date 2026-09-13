use crate::brain::{Brain, BrainConfig, BrainSnapshot};
use crate::physics::{Food, Fly};

pub const NEURAL_DT: f32 = 0.002;

#[derive(Debug)]
pub struct Simulation {
    pub brain: Brain,
    pub fly: Fly,
    pub food: Food,
    reward_cooldown: f32,
    rng_state: u32,
}

impl Simulation {
    pub fn new() -> Self {
        Self {
            brain: Brain::new(BrainConfig::default()),
            fly: Fly::default(),
            food: Food::default(),
            reward_cooldown: 0.0,
            rng_state: 0x1357_9BDF,
        }
    }

    pub fn step(&mut self) -> BrainSnapshot {
        if self.reward_cooldown > 0.0 {
            self.reward_cooldown = (self.reward_cooldown - NEURAL_DT).max(0.0);
        }

        let visual = self.fly.visual_input(&self.food);
        let snapshot = self.brain.step(visual, NEURAL_DT);
        self.fly.apply_motor(snapshot.turn, snapshot.forward, NEURAL_DT);
        self.fly.integrate(NEURAL_DT);

        if self.reward_cooldown == 0.0 && self.fly.collides_with(&self.food) {
            self.brain.inject_dopamine(1.0);
            self.respawn_food();
            self.reward_cooldown = 0.2;
        }

        snapshot
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
        let x = (self.next_u32() as f32 / u32::MAX as f32) * 12.0 - 6.0;
        let z = (self.next_u32() as f32 / u32::MAX as f32) * 12.0 - 6.0;
        self.food.position = [x, 0.0, z];
    }
}

impl Default for Simulation {
    fn default() -> Self {
        Self::new()
    }
}
