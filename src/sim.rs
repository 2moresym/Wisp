use crate::brain::{Brain, BrainConfig, BrainSnapshot};
use crate::physics::{Food, Fly, ARENA_HALF_SIZE};

pub const NEURAL_DT: f32 = 0.002;
const REWARD_COOLDOWN: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default)]
pub struct SimulationStats { pub steps:u64, pub rewards:u64, pub last_turn:f32, pub last_forward:f32 }

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
    pub fn new()->Self { Self { brain:Brain::new(BrainConfig::default()), fly:Fly::default(), food:Food::default(), reward_cooldown:0.0, rng_state:0x1357_9BDF, stats:SimulationStats::default() } }
    pub fn step(&mut self)->BrainSnapshot {
        self.reward_cooldown=(self.reward_cooldown-NEURAL_DT).max(0.0);
        let visual=self.fly.visual_input(&self.food);
        let snapshot=self.brain.step(visual,NEURAL_DT);
        self.fly.apply_motor(snapshot.turn,snapshot.forward,NEURAL_DT);
        self.fly.integrate(NEURAL_DT);
        if self.reward_cooldown==0.0 && self.fly.collides_with(&self.food) {
            self.brain.inject_dopamine(1.0); self.stats.rewards+=1; self.respawn_food(); self.reward_cooldown=REWARD_COOLDOWN;
        }
        self.stats.steps+=1; self.stats.last_turn=snapshot.turn; self.stats.last_forward=snapshot.forward; snapshot
    }
    pub fn steps(&mut self,count:usize){for _ in 0..count{self.step();}}
    pub fn force_food_at_fly(&mut self){self.food.position=self.fly.position;}
    fn next_u32(&mut self)->u32{let mut x=self.rng_state;x^=x<<13;x^=x>>17;x^=x<<5;self.rng_state=x;x}
    fn respawn_food(&mut self){
        let span=(ARENA_HALF_SIZE-1.0)*2.0;
        let x=(self.next_u32() as f32/u32::MAX as f32)*span-(ARENA_HALF_SIZE-1.0);
        let z=(self.next_u32() as f32/u32::MAX as f32)*span-(ARENA_HALF_SIZE-1.0);
        self.food.position=[x,0.35,z];
        if self.fly.collides_with(&self.food){self.food.position[0]+=1.5;}
    }
}
impl Default for Simulation{fn default()->Self{Self::new()}}

#[cfg(test)] mod tests{
 use super::*;
 #[test] fn deterministic(){let mut a=Simulation::default();let mut b=Simulation::default();for _ in 0..2000{a.step();b.step();}assert_eq!(a.stats.steps,b.stats.steps);assert_eq!(a.stats.rewards,b.stats.rewards);assert_eq!(a.brain.weight_checksum(),b.brain.weight_checksum());assert_eq!(a.fly.position,b.fly.position);}
 #[test] fn collision_rewards(){let mut s=Simulation::default();s.force_food_at_fly();s.step();assert_eq!(s.stats.rewards,1);}
}
