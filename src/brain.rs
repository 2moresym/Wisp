//! Cache-friendly reduced neural model.
//!
//! Layout goals:
//! - neuron state is stored in flat contiguous vectors;
//! - sparse synapses use compact CSR-like incoming adjacency;
//! - only the next-step spike/current arrays are touched by the hot loop;
//! - Rayon parallelizes independent neuron updates;
//! - plasticity is kept out of the parallel state update to avoid locks.

use rayon::prelude::*;

pub const NEURONS: usize = 5_000;
pub const VISUAL_START: usize = 0;
pub const VISUAL_COUNT: usize = 1_024;
pub const MB_START: usize = 1_024;
pub const MB_COUNT: usize = 2_976;
pub const MOTOR_START: usize = 4_700;
pub const MOTOR_COUNT: usize = 300;
pub const PAM_COUNT: usize = 32;

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
    pub neuron_count: usize,
    pub fan_in: usize,
    pub leak: f32,
    pub threshold: f32,
    pub reset: f32,
}

impl Default for BrainConfig {
    fn default() -> Self {
        Self {
            neuron_count: NEURONS,
            fan_in: 12,
            leak: 0.94,
            threshold: 1.0,
            reset: 0.0,
        }
    }
}

pub struct Brain {
    membrane: Vec<f32>,
    previous_membrane: Vec<f32>,
    spikes: Vec<u8>,
    next_spikes: Vec<u8>,
    input_current: Vec<f32>,
    traces: Vec<f32>,

    // Incoming CSR-like storage. For neuron n, edges are in
    // weights[offsets[n]..offsets[n + 1]], with matching source IDs.
    offsets: Vec<u32>,
    sources: Vec<u16>,
    weights: Vec<f32>,

    dopamine: f32,
    learning_rate: f32,
    step_index: u64,
}

impl Brain {
    pub fn new(config: BrainConfig) -> Self {
        assert_eq!(config.neuron_count, NEURONS, "initial topology is fixed at 5k neurons");

        let mut offsets = Vec::with_capacity(NEURONS + 1);
        let mut sources = Vec::with_capacity(NEURONS * config.fan_in);
        let mut weights = Vec::with_capacity(NEURONS * config.fan_in);
        offsets.push(0);

        // Deterministic xorshift topology: no RNG allocation and identical runs.
        let mut seed = 0xC0FF_EE12_u32;
        for target in 0..NEURONS {
            for _ in 0..config.fan_in {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let source = (seed as usize % NEURONS) as u16;
                sources.push(source);
                // Small positive excitatory baseline. A few inhibitory-like
                // edges are represented by negative weights.
                let magnitude = 0.035 + ((seed & 0xFF) as f32 / 255.0) * 0.045;
                weights.push(if (seed & 0x100) != 0 { magnitude } else { -magnitude * 0.55 });
            }
            offsets.push(weights.len() as u32);

            let _ = target;
        }

        Self {
            membrane: vec![0.0; NEURONS],
            previous_membrane: vec![0.0; NEURONS],
            spikes: vec![0; NEURONS],
            next_spikes: vec![0; NEURONS],
            input_current: vec![0.0; NEURONS],
            traces: vec![0.0; NEURONS],
            offsets,
            sources,
            weights,
            dopamine: 0.0,
            learning_rate: 0.015,
            step_index: 0,
        }
    }

    pub fn step(&mut self, visual: VisualInput, dt: f32) -> BrainSnapshot {
        self.encode_visual(visual);

        // Integrate incoming spikes. This writes each target independently, so
        // Rayon can split the contiguous target range without synchronization.
        self.input_current
            .par_iter_mut()
            .enumerate()
            .for_each(|(target, current)| {
                let begin = self.offsets[target] as usize;
                let end = self.offsets[target + 1] as usize;
                let mut sum = 0.0;
                for edge in begin..end {
                    sum += self.weights[edge] * self.spikes[self.sources[edge] as usize] as f32;
                }
                *current += sum;
            });

        let leak = 0.94_f32.powf(dt * 500.0);
        let threshold = 1.0;
        let reset = 0.0;
        let current = &self.input_current;
        let previous = &self.membrane;

        self.next_spikes
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, spike)| {
                let v = previous[i] * leak + current[i];
                *spike = u8::from(v >= threshold);
            });

        std::mem::swap(&mut self.spikes, &mut self.next_spikes);
        self.previous_membrane.copy_from_slice(&self.membrane);

        self.membrane
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, membrane)| {
                let v = self.previous_membrane[i] * leak + self.input_current[i];
                *membrane = if self.spikes[i] != 0 { reset } else { v };
            });

        self.traces.par_iter_mut().enumerate().for_each(|(i, trace)| {
            *trace *= 0.985;
            if self.spikes[i] != 0 {
                *trace += 1.0;
            }
        });

        // Basic Hebbian eligibility: co-active source/target pairs receive a
        // dopamine-gated update. No allocation is performed in the hot path.
        if self.dopamine > 0.001 {
            self.apply_dopamine_plasticity();
            self.dopamine *= 0.995;
        }

        self.input_current.fill(0.0);
        self.step_index += 1;
        self.motor_snapshot()
    }

    fn encode_visual(&mut self, visual: VisualInput) {
        let normalized_angle = (visual.angle / std::f32::consts::PI).clamp(-1.0, 1.0);
        let proximity = (1.0 - visual.distance / 20.0).clamp(0.0, 1.0);

        // Population code: left/right and proximity drive disjoint visual bands.
        let left = ((1.0 - normalized_angle) * 0.5 * (VISUAL_COUNT as f32 - 1.0)) as usize;
        let right = ((1.0 + normalized_angle) * 0.5 * (VISUAL_COUNT as f32 - 1.0)) as usize;
        self.input_current[VISUAL_START + left] += 1.25 * proximity;
        self.input_current[VISUAL_START + right] += 1.25 * proximity;
    }

    fn apply_dopamine_plasticity(&mut self) {
        let reward = self.dopamine.clamp(0.0, 2.0);
        let mut changed = 0usize;

        // Restrict learning to visual -> mushroom-body/motor-relevant edges.
        for target in MB_START..(MB_START + MB_COUNT) {
            let begin = self.offsets[target] as usize;
            let end = self.offsets[target + 1] as usize;
            for edge in begin..end {
                let source = self.sources[edge] as usize;
                if source < VISUAL_START + VISUAL_COUNT && self.traces[source] > 0.05 {
                    self.weights[edge] += self.learning_rate * reward * self.traces[source];
                    self.weights[edge] = self.weights[edge].clamp(-1.0, 1.0);
                    changed += 1;
                }
            }
        }

        if changed != 0 {
            println!(
                "[WISP] dopamine={:.3} -> potentiated {} visual synapses",
                reward, changed
            );
        }
    }

    fn motor_snapshot(&self) -> BrainSnapshot {
        let mut left = 0.0;
        let mut right = 0.0;
        let mut forward = 0.0;

        for i in 0..MOTOR_COUNT {
            let activity = self.spikes[MOTOR_START + i] as f32;
            match i % 3 {
                0 => left += activity,
                1 => right += activity,
                _ => forward += activity,
            }
        }

        BrainSnapshot {
            turn: ((right - left) / 20.0).clamp(-1.0, 1.0),
            forward: (forward / 20.0).clamp(0.0, 1.0),
        }
    }

    /// Artificial reward signal representing activation of the simulated PAM
    /// dopaminergic cluster. Reward persists through synaptic weight changes.
    pub fn inject_dopamine(&mut self, reward: f32) {
        self.dopamine = (self.dopamine + reward.clamp(0.0, 2.0)).min(2.0);
        println!(
            "[WISP] PAM dopamine spike: reward={:.2}, level={:.3}",
            reward, self.dopamine
        );
    }

    #[allow(dead_code)]
    pub fn dopamine(&self) -> f32 {
        self.dopamine
    }

    #[allow(dead_code)]
    pub fn neuron_count(&self) -> usize {
        NEURONS
    }
}
