use wisp::brain::{Brain, BrainConfig, VisualInput};
use wisp::physics::{Food, Fly};
use wisp::sim::Simulation;

#[test]
fn simulation_is_deterministic() {
    let mut a = Simulation::default();
    let mut b = Simulation::default();
    for _ in 0..2_000 {
        a.step();
        b.step();
    }
    assert_eq!(a.stats.steps, b.stats.steps);
    assert_eq!(a.stats.rewards, b.stats.rewards);
    assert_eq!(a.fly.position, b.fly.position);
    assert_eq!(a.brain.weight_checksum(), b.brain.weight_checksum());
}

#[test]
fn reward_changes_persistent_weights() {
    let mut brain = Brain::new(BrainConfig::default());
    brain.step(VisualInput { angle: 0.2, distance: 1.0, lateral: 0.1 }, 0.002);
    let before = brain.weight_checksum();
    brain.inject_dopamine(1.0);
    assert_ne!(before, brain.weight_checksum());
}

#[test]
fn collision_geometry_is_consistent() {
    let fly = Fly::default();
    let mut food = Food::default();
    food.position = fly.position;
    assert!(fly.collides_with(&food));
    food.position[0] += 10.0;
    assert!(!fly.collides_with(&food));
}
