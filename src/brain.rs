//! Compact deterministic 5,000-neuron Drosophila-inspired subgraph.
//!
//! Engineering abstraction: flat state arrays + sparse layered connectivity.

use rayon::prelude::*;

pub const NEURONS: usize = 5_000;
pub const VISUAL_START: usize = 0;
pub const VISUAL_COUNT: usize = 1_024;
pub const KENYON_START: usize = 1_024;
pub const KENYON_COUNT: usize = 2_560;
pub const MBON_START: usize = 3_584;
pub const MBON_COUNT: usize = 512;
pub const MOTOR_START: usize = 4_096;
pub const MOTOR_LEFT_COUNT: usize = 64;
pub const MOTOR_RIGHT_START: usize = 4_160;
pub const MOTOR_RIGHT_COUNT: usize = 64;
pub const MOTOR_FORWARD_START: usize = 4_224;
pub const MOTOR_FORWARD_COUNT: usize = 128;
pub const PAM_START: usize = 4_352;
pub const PAM_COUNT: usize = 32;
pub const REST_START: usize = 4_384;
pub const REST_COUNT: usize = 616;
pub const FAN_IN: usize = 12;
pub const MIN_WEIGHT: f32 = -1.0;
pub const MAX_WEIGHT: f32 = 1.0;
const VISUAL_SECTORS: usize = 32;
const VISUAL_CHANNELS: usize = VISUAL_COUNT / VISUAL_SECTORS;

#[derive(Clone, Copy, Debug, Default)]
pub struct VisualInput {
    pub angle: f32,
    pub distance: f32,
    pub lateral: f32,
    pub approach_velocity: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BrainSnapshot {
    pub turn: f32,
    pub forward: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BrainConfig {
    pub fan_in: usize,
    pub tau_ms: f32,
    pub threshold: f32,
    pub reset: f32,
    pub refractory_steps: u8,
    pub trace_decay: f32,
    pub eligibility_decay: f32,
    pub dopamine_decay: f32,
    pub learning_rate: f32,
}

impl Default for BrainConfig {
    fn default() -> Self {
        Self {
            fan_in: FAN_IN,
            tau_ms: 20.0,
            threshold: 1.0,
            reset: 0.0,
            refractory_steps: 2,
            trace_decay: 0.985,
            eligibility_decay: 0.98,
            dopamine_decay: 0.995,
            learning_rate: 0.015,
        }
    }
}

#[derive(Debug)]
pub struct Brain {
    membrane: Vec<f32>,
    spikes: Vec<u8>,
    next_spikes: Vec<u8>,
    refractory: Vec<u8>,
    input_current: Vec<f32>,
    traces: Vec<f32>,
    offsets: Vec<u32>,
    sources: Vec<u16>,
    edge_targets: Vec<u16>,
    weights: Vec<f32>,
    eligibility: Vec<f32>,
    dopamine: f32,
    dopamine_events: u64,
    config: BrainConfig,
    step_index: u64,
    last_changed: usize,
    last_mean_before: f32,
    last_mean_after: f32,
}

impl Brain {
    pub fn new(config: BrainConfig) -> Self {
        assert!(config.fan_in > 0);
        assert!(NEURONS <= u16::MAX as usize + 1);
        let edges = NEURONS * config.fan_in;
        let mut offsets = Vec::with_capacity(NEURONS + 1);
        let mut sources = Vec::with_capacity(edges);
        let mut edge_targets = Vec::with_capacity(edges);
        let mut weights = Vec::with_capacity(edges);
        offsets.push(0);
        for target in 0..NEURONS {
            for edge in 0..config.fan_in {
                let seed = xorshift(
                    (target as u32).wrapping_mul(0x9E37_79B9)
                        ^ (edge as u32).wrapping_mul(0x85EB_CA6B)
                        ^ 0xC0FF_EE12,
                );
                sources.push(layered_source(target, edge) as u16);
                edge_targets.push(target as u16);
                weights.push(feedforward_weight(target, edge, seed));
            }
            offsets.push(sources.len() as u32);
        }
        Self {
            membrane: vec![0.0; NEURONS],
            spikes: vec![0; NEURONS],
            next_spikes: vec![0; NEURONS],
            refractory: vec![0; NEURONS],
            input_current: vec![0.0; NEURONS],
            traces: vec![0.0; NEURONS],
            offsets,
            sources,
            edge_targets,
            weights,
            eligibility: vec![0.0; edges],
            dopamine: 0.0,
            dopamine_events: 0,
            config,
            step_index: 0,
            last_changed: 0,
            last_mean_before: 0.0,
            last_mean_after: 0.0,
        }
    }

    pub fn step(&mut self, visual: VisualInput, dt: f32) -> BrainSnapshot {
        self.encode_visual(visual);
        let offsets = &self.offsets;
        let sources = &self.sources;
        let weights = &self.weights;
        let spikes = &self.spikes;
        self.input_current
            .par_iter_mut()
            .enumerate()
            .for_each(|(target, current)| {
                let begin = offsets[target] as usize;
                let end = offsets[target + 1] as usize;
                let mut sum = *current;
                for edge in begin..end {
                    sum += weights[edge] * spikes[sources[edge] as usize] as f32;
                }
                *current = sum;
            });

        let leak = (-dt / (self.config.tau_ms * 0.001)).exp();
        let threshold = self.config.threshold;
        let reset = self.config.reset;
        let refractory_steps = self.config.refractory_steps;
        let current = &self.input_current;
        let membrane = &self.membrane;
        let refractory = &self.refractory;
        self.next_spikes
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, spike)| {
                *spike = u8::from(
                    refractory[i] == 0 && membrane[i] * leak + current[i] >= threshold,
                );
            });
        let next_spikes = &self.next_spikes;
        self.membrane
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, value)| {
                if refractory[i] != 0 {
                    *value = reset;
                } else {
                    *value = if next_spikes[i] != 0 {
                        reset
                    } else {
                        *value * leak + current[i]
                    };
                }
            });
        std::mem::swap(&mut self.spikes, &mut self.next_spikes);
        let spikes = &self.spikes;
        self.refractory
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, value)| {
                if spikes[i] != 0 {
                    *value = refractory_steps;
                } else if *value != 0 {
                    *value -= 1;
                }
            });
        let spikes = &self.spikes;
        self.traces
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, trace)| {
                *trace *= self.config.trace_decay;
                if spikes[i] != 0 {
                    *trace = (*trace + 1.0).min(4.0);
                }
            });
        let spikes = &self.spikes;
        let traces = &self.traces;
        let targets = &self.edge_targets;
        self.eligibility
            .par_iter_mut()
            .enumerate()
            .for_each(|(edge, e)| {
                *e *= self.config.eligibility_decay;
                let target = targets[edge] as usize;
                let source = sources[edge] as usize;
                if spikes[target] != 0 && spikes[source] != 0 {
                    *e = (*e + 1.0).min(4.0);
                } else if spikes[target] != 0 && traces[source] > 0.1 {
                    *e = (*e + 0.25).min(4.0);
                }
            });
        if self.dopamine > 0.001 {
            self.apply_dopamine_plasticity();
            self.dopamine *= self.config.dopamine_decay;
            if self.dopamine < 0.001 {
                self.dopamine = 0.0;
            }
        }
        self.input_current.fill(0.0);
        self.step_index += 1;
        self.motor_snapshot()
    }

    fn encode_visual(&mut self, visual: VisualInput) {
        let angle = (visual.angle / std::f32::consts::PI).clamp(-1.0, 1.0);
        let proximity = (1.0 - visual.distance / 20.0).clamp(0.0, 1.0);
        let lateral = visual.lateral.clamp(-1.0, 1.0);
        let approach = ((-visual.approach_velocity) / 4.0).clamp(-1.0, 1.0);
        let sector_position = ((angle * 0.5 + 0.5) * (VISUAL_SECTORS - 1) as f32)
            .round()
            .clamp(0.0, (VISUAL_SECTORS - 1) as f32) as usize;
        let lateral_shift = (lateral * 2.0).round() as isize;
        let sector = (sector_position as isize + lateral_shift)
            .clamp(0, VISUAL_SECTORS as isize - 1) as usize;
        let drive = 1.65 + 1.25 * proximity;
        let base = VISUAL_START + sector * VISUAL_CHANNELS;
        for channel in 0..VISUAL_CHANNELS {
            let gain = if channel < 4 {
                drive
            } else if channel < 8 {
                drive * proximity
            } else if channel < 12 {
                drive * approach.max(0.0)
            } else {
                drive * (0.5 + 0.5 * approach)
            };
            self.input_current[base + channel] += gain;
        }
    }

    fn apply_dopamine_plasticity(&mut self) {
        let reward = self.dopamine.clamp(0.0, 2.0);
        let mut changed = 0usize;
        let mut before = 0.0;
        let mut after = 0.0;
        for edge in 0..self.weights.len() {
            let target = self.edge_targets[edge] as usize;
            let source = self.sources[edge] as usize;
            let visual = source < VISUAL_COUNT && self.traces[source] > 0.05;
            let eligibility = self.eligibility[edge];
            let signal = if eligibility > 0.001 {
                eligibility
            } else if visual && (KENYON_START..MBON_START).contains(&target) {
                self.traces[source]
            } else {
                0.0
            };
            if signal <= 0.0 {
                continue;
            }
            let old = self.weights[edge];
            let new = (old + self.config.learning_rate * reward * signal)
                .clamp(MIN_WEIGHT, MAX_WEIGHT);
            if (new - old).abs() > f32::EPSILON {
                self.weights[edge] = new;
                before += old;
                after += new;
                changed += 1;
            }
        }
        self.last_changed = changed;
        self.last_mean_before = if changed == 0 {
            0.0
        } else {
            before / changed as f32
        };
        self.last_mean_after = if changed == 0 {
            0.0
        } else {
            after / changed as f32
        };
    }

    fn motor_snapshot(&self) -> BrainSnapshot {
        let left = spike_sum(&self.spikes[MOTOR_START..MOTOR_RIGHT_START]);
        let right = spike_sum(&self.spikes[MOTOR_RIGHT_START..MOTOR_FORWARD_START]);
        let forward = spike_sum(&self.spikes[MOTOR_FORWARD_START..PAM_START]);
        BrainSnapshot {
            turn: ((right - left) / MOTOR_LEFT_COUNT as f32).clamp(-1.0, 1.0),
            forward: (forward / MOTOR_FORWARD_COUNT as f32).clamp(0.0, 1.0),
        }
    }

    pub fn inject_dopamine(&mut self, reward: f32) {
        let reward = reward.clamp(0.0, 2.0);
        self.dopamine = (self.dopamine + reward).min(2.0);
        self.spikes[PAM_START..PAM_START + PAM_COUNT].fill(1);
        self.dopamine_events += 1;
        self.apply_dopamine_plasticity();
    }

    pub fn dopamine(&self) -> f32 { self.dopamine }
    pub fn dopamine_events(&self) -> u64 { self.dopamine_events }
    pub fn neuron_count(&self) -> usize { NEURONS }
    pub fn synapse_count(&self) -> usize { self.weights.len() }
    pub fn last_changed_synapses(&self) -> usize { self.last_changed }
    pub fn last_mean_weight_before(&self) -> f32 { self.last_mean_before }
    pub fn last_mean_weight_after(&self) -> f32 { self.last_mean_after }
    pub fn step_index(&self) -> u64 { self.step_index }
    pub fn weight_checksum(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for &w in &self.weights {
            for b in w.to_bits().to_le_bytes() {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }
    pub fn memory_bytes(&self) -> usize {
        self.membrane.len() * 4
            + self.spikes.len()
            + self.next_spikes.len()
            + self.refractory.len()
            + self.input_current.len() * 4
            + self.traces.len() * 4
            + self.offsets.len() * 4
            + self.sources.len() * 2
            + self.edge_targets.len() * 2
            + self.weights.len() * 4
            + self.eligibility.len() * 4
    }
}

fn layered_source(target: usize, edge: usize) -> usize {
    match target {
        KENYON_START..MBON_START => {
            let sector = (target - KENYON_START) % VISUAL_SECTORS;
            VISUAL_START + sector * VISUAL_CHANNELS + (edge * 7) % VISUAL_CHANNELS
        }
        MBON_START..MOTOR_START => {
            let sector = (target - MBON_START) % VISUAL_SECTORS;
            KENYON_START + sector * (KENYON_COUNT / VISUAL_SECTORS)
                + (edge * 17) % (KENYON_COUNT / VISUAL_SECTORS)
        }
        MOTOR_START..PAM_START => {
            let motor_index = target - MOTOR_START;
            let sector = match motor_index {
                0..MOTOR_LEFT_COUNT => (motor_index % 16) as isize,
                MOTOR_LEFT_COUNT..{ MOTOR_LEFT_COUNT + MOTOR_RIGHT_COUNT } => {
                    31 - (motor_index % 16) as isize
                }
                _ => 8 + (motor_index % 16) as isize,
            } as usize;
            MBON_START + sector * (MBON_COUNT / VISUAL_SECTORS) + (edge * 5) % (MBON_COUNT / VISUAL_SECTORS)
        }
        PAM_START.. => MBON_START + ((target - PAM_START) * 19 + edge * 23) % MBON_COUNT,
        _ => VISUAL_START + (target * 5 + edge * 7) % VISUAL_COUNT,
    }
}

fn feedforward_weight(target: usize, edge: usize, seed: u32) -> f32 {
    let random = 0.12 + ((seed & 0xFF) as f32 / 255.0) * 0.08;
    let gain = if (KENYON_START..MBON_START).contains(&target) {
        1.0
    } else if (MBON_START..MOTOR_START).contains(&target) {
        1.0
    } else if (MOTOR_START..PAM_START).contains(&target) {
        1.2
    } else {
        0.8
    };
    let sign = if (seed & 0x400) != 0 && edge % 5 == 0 { -0.35 } else { 1.0 };
    random * gain * sign
}

fn spike_sum(values: &[u8]) -> f32 { values.iter().map(|&v| v as f32).sum() }
fn xorshift(mut x: u32) -> u32 {
    if x == 0 { x = 0xA341_316C; }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_is_compact() {
        let b = Brain::new(BrainConfig::default());
        assert_eq!(b.neuron_count(), 5000);
        assert_eq!(b.synapse_count(), 60000);
        assert!(b.memory_bytes() < 2_000_000);
    }

    #[test]
    fn deterministic_topology() {
        let a = Brain::new(BrainConfig::default());
        let b = Brain::new(BrainConfig::default());
        assert_eq!(a.weight_checksum(), b.weight_checksum());
    }

    #[test]
    fn dopamine_changes_weights() {
        let mut b = Brain::new(BrainConfig::default());
        for _ in 0..8 {
            b.step(
                VisualInput { angle: 0.3, distance: 2.0, lateral: 0.2, approach_velocity: -1.0 },
                0.002,
            );
        }
        let old = b.weight_checksum();
        b.inject_dopamine(1.0);
        assert_ne!(old, b.weight_checksum());
        assert!(b.last_changed_synapses() > 0);
    }

    #[test]
    fn lif_neuron_reaches_threshold() {
        let mut b = Brain::new(BrainConfig::default());
        let mut saw_spike = false;
        for _ in 0..20 {
            b.step(
                VisualInput { angle: 0.0, distance: 0.0, lateral: 0.0, approach_velocity: 0.0 },
                0.002,
            );
            saw_spike |= b.traces[0] > 0.0;
        }
        assert!(saw_spike);
    }
}
