//! Compact, deterministic 5,000-neuron Drosophila-inspired subgraph.
//!
//! The implementation favors structure-of-arrays storage and sequential CSR
//! synapses over object-heavy neuron graphs. It is an engineering abstraction,
//! not a complete biological reconstruction.

use rayon::prelude::*;

pub const NEURONS: usize = 5_000;
pub const VISUAL_START: usize = 0;
pub const VISUAL_COUNT: usize = 1_024;
pub const KENYON_START: usize = VISUAL_START + VISUAL_COUNT;
pub const KENYON_COUNT: usize = 2_560;
pub const MBON_START: usize = KENYON_START + KENYON_COUNT;
pub const MBON_COUNT: usize = 512;
pub const MOTOR_START: usize = MBON_START + MBON_COUNT;
pub const MOTOR_LEFT_COUNT: usize = 64;
pub const MOTOR_RIGHT_START: usize = MOTOR_START + MOTOR_LEFT_COUNT;
pub const MOTOR_RIGHT_COUNT: usize = 64;
pub const MOTOR_FORWARD_START: usize = MOTOR_RIGHT_START + MOTOR_RIGHT_COUNT;
pub const MOTOR_FORWARD_COUNT: usize = 128;
pub const PAM_START: usize = MOTOR_FORWARD_START + MOTOR_FORWARD_COUNT;
pub const PAM_COUNT: usize = 32;
pub const REST_START: usize = PAM_START + PAM_COUNT;
pub const REST_COUNT: usize = NEURONS - REST_START;

pub const FAN_IN: usize = 12;
pub const MIN_WEIGHT: f32 = -1.0;
pub const MAX_WEIGHT: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct VisualInput {
    pub angle: f32,
    pub distance: f32,
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

pub struct Brain {
    membrane: Vec<f32>,
    spikes: Vec<u8>,
    next_spikes: Vec<u8>,
    refractory: Vec<u8>,
    input_current: Vec<f32>,
    traces: Vec<f32>,
    offsets: Vec<u32>,
    sources: Vec<u16>,
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
        assert!(NEURONS <= u16::MAX as usize + 1, "u16 source IDs overflow");

        let mut offsets = Vec::with_capacity(NEURONS + 1);
        let mut sources = Vec::with_capacity(NEURONS * config.fan_in);
        let mut weights = Vec::with_capacity(NEURONS * config.fan_in);
        offsets.push(0);

        for target in 0..NEURONS {
            for edge in 0..config.fan_in {
                let source = layered_source(target, edge) as u16;
                let seed = xorshift((target as u32).wrapping_mul(0x9E37_79B9) ^ edge as u32 ^ 0xC0FF_EE12);
                let magnitude = 0.035 + ((seed & 0xFF) as f32 / 255.0) * 0.045;
                let sign = if (seed & 0x100) != 0 { 1.0 } else { -0.55 };
                sources.push(source);
                weights.push(magnitude * sign);
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
            eligibility: vec![0.0; weights.len()],
            offsets,
            sources,
            weights,
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
                if refractory[i] != 0 {
                    *spike = 0;
                } else {
                    *spike = u8::from(membrane[i] * leak + current[i] >= threshold);
                }
            });

        let next_spikes = &self.next_spikes;
        self.membrane
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, value)| {
                if refractory[i] != 0 {
                    *value = reset;
                } else {
                    let integrated = *value * leak + current[i];
                    *value = if next_spikes[i] != 0 { reset } else { integrated };
                }
            });

        std::mem::swap(&mut self.spikes, &mut self.next_spikes);

        self.refractory
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, value)| {
                if self.spikes[i] != 0 {
                    *value = refractory_steps;
                } else if *value != 0 {
                    *value -= 1;
                }
            });

        self.traces.par_iter_mut().enumerate().for_each(|(i, trace)| {
            *trace *= self.config.trace_decay;
            if self.spikes[i] != 0 {
                *trace += 1.0;
            }
        });

        let spikes = &self.spikes;
        let traces = &self.traces;
        self.eligibility
            .par_iter_mut()
            .enumerate()
            .for_each(|(edge, eligibility)| {
                *eligibility *= self.config.eligibility_decay;
                let target = edge_target(edge, offsets);
                let source = sources[edge] as usize;
                if spikes[target] != 0 && spikes[source] != 0 {
                    *eligibility = (*eligibility + 1.0).min(4.0);
                } else if traces[source] > 0.1 && spikes[target] != 0 {
                    *eligibility = (*eligibility + 0.25).min(4.0);
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
        let normalized = (visual.angle / std::f32::consts::PI).clamp(-1.0, 1.0);
        let proximity = (1.0 - visual.distance / 20.0).clamp(0.0, 1.0);
        let center = VISUAL_COUNT / 2;
        let offset = (normalized * (center as f32 - 1.0)) as isize;
        let left = (center as isize - offset).clamp(0, VISUAL_COUNT as isize - 1) as usize;
        let right = (center as isize + offset).clamp(0, VISUAL_COUNT as isize - 1) as usize;
        let drive = 1.35 * proximity;
        self.input_current[VISUAL_START + left] += drive;
        self.input_current[VISUAL_START + right] += drive;
    }

    fn apply_dopamine_plasticity(&mut self) {
        let reward = self.dopamine.clamp(0.0, 2.0);
        let mut changed = 0usize;
        let mut before_sum = 0.0f32;
        let mut after_sum = 0.0f32;

        for edge in 0..self.weights.len() {
            let target = edge_target(edge, &self.offsets);
            let source = self.sources[edge] as usize;
            let eligibility = self.eligibility[edge];
            let visual_trace = source < VISUAL_START + VISUAL_COUNT && self.traces[source] > 0.05;
            let learning_signal = if eligibility > 0.001 { eligibility } else if visual_trace && target >= KENYON_START && target < MBON_START { self.traces[source] } else { 0.0 };
            if learning_signal <= 0.0 {
                continue;
            }
            let old = self.weights[edge];
            let delta = self.config.learning_rate * reward * learning_signal;
            let new = (old + delta).clamp(MIN_WEIGHT, MAX_WEIGHT);
            if (new - old).abs() > f32::EPSILON {
                before_sum += old;
                after_sum += new;
                self.weights[edge] = new;
                changed += 1;
            }
        }

        self.last_changed = changed;
        self.last_mean_before = if changed == 0 { 0.0 } else { before_sum / changed as f32 };
        self.last_mean_after = if changed == 0 { 0.0 } else { after_sum / changed as f32 };
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

    /// Activate the simulated PAM dopaminergic cluster and apply reward learning.
    pub fn inject_dopamine(&mut self, reward: f32) {
        let reward = reward.clamp(0.0, 2.0);
        self.dopamine = (self.dopamine + reward).min(2.0);
        self.spikes[PAM_START..PAM_START + PAM_COUNT].fill(1);
        self.dopamine_events += 1;
        self.apply_dopamine_plasticity();
        println!(
            "[DOPAMINE] reward={reward:.2} PAM spike | changed={} | mean weight {:.4} -> {:.4}",
            self.last_changed, self.last_mean_before, self.last_mean_after
        );
    }

    pub fn dopamine(&self) -> f32 { self.dopamine }
    pub fn dopamine_events(&self) -> u64 { self.dopamine_events }
    pub fn neuron_count(&self) -> usize { NEURONS }
    pub fn synapse_count(&self) -> usize { self.weights.len() }
    pub fn last_changed_synapses(&self) -> usize { self.last_changed }

    pub fn weight_checksum(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for &weight in &self.weights {
            for byte in weight.to_bits().to_le_bytes() {
                hash ^= byte as u64;
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
            + self.weights.len() * 4
            + self.eligibility.len() * 4
    }
}

fn layered_source(target: usize, edge: usize) -> usize {
    match target {
        KENYON_START..MBON_START => VISUAL_START + ((target - KENYON_START) * 17 + edge * 31) % VISUAL_COUNT,
        MBON_START..MOTOR_START => KENYON_START + ((target - MBON_START) * 13 + edge * 29) % KENYON_COUNT,
        MOTOR_START..PAM_START => MBON_START + ((target - MOTOR_START) * 7 + edge * 11) % MBON_COUNT,
        PAM_START.. => MBON_START + ((target - PAM_START) * 19 + edge * 23) % MBON_COUNT,
        _ => VISUAL_START + ((target * 5 + edge * 7) % VISUAL_COUNT),
    }
}

fn edge_target(edge: usize, offsets: &[u32]) -> usize {
    let mut low = 0usize;
    let mut high = offsets.len() - 1;
    while low < high {
        let mid = (low + high) / 2;
        if offsets[mid + 1] as usize <= edge { low = mid + 1; } else { high = mid; }
    }
    low
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
    fn topology_is_layered() {
        let brain = Brain::new(BrainConfig::default());
        assert_eq!(brain.neuron_count(), 5_000);
        assert_eq!(brain.synapse_count(), 5_000 * FAN_IN);
    }

    #[test]
    fn dopamine_changes_weights() {
        let mut brain = Brain::new(BrainConfig::default());
        brain.step(VisualInput { angle: 0.3, distance: 2.0 }, 0.002);
        let before = brain.weight_checksum();
        brain.inject_dopamine(1.0);
        assert_ne!(before, brain.weight_checksum());
    }

    #[test]
    fn weights_stay_clamped() {
        let mut brain = Brain::new(BrainConfig::default());
        brain.step(VisualInput { angle: 0.0, distance: 1.0 }, 0.002);
        for _ in 0..100 { brain.inject_dopamine(2.0); }
        assert!(brain.weights.iter().all(|w| (*w >= MIN_WEIGHT) && (*w <= MAX_WEIGHT)));
    }
}
