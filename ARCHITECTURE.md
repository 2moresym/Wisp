# Wisp Architecture

Wisp is a small deterministic research simulator combining a reduced Drosophila-inspired neural subgraph with a low-poly 3D arena. It is a computational emulator, not a full reconstruction of the fly brain.

## Runtime flow

```text
Food + Fly state
      │
      ▼
lightweight visual encoding
(angle, distance, lateral bearing, approach velocity)
      │
      ▼
VISION 1024
      │
      ▼
KENYON 2560
      │
      ▼
MBON 512
      │
      ▼
MOTOR 256 ───────► Fly turn + forward thrust
      │
      └──────────► PAM 32 / reward learning
                         ▲
                         │
                    food collision
```

The simulation advances in a fixed 2 ms neural step. Rendering runs independently and never feeds pixels back into the neural model.

## Brain storage

`brain.rs` is data-oriented. Hot state lives in contiguous arrays:

- membrane potential: `f32`
- current: `f32`
- activity/traces: `f32`
- weights: `f32`
- eligibility: `f32`
- spikes/refractory: compact integer arrays
- sparse source indices: `u16`
- CSR-style target offsets: `u32`

The default topology has 5,000 neurons and 60,000 edges. Core brain storage is explicitly accounted by `Brain::memory_bytes()` and is about 1 MiB.

## Topology

The topology is deterministic rather than randomly regenerated at runtime. Visual sectors are preserved across the sensory pathway:

1. Visual neurons are arranged as 32 spatial sectors with 32 channels each.
2. Kenyon cells sample within their corresponding visual sector.
3. MBON cells preserve the same sector grouping.
4. Left, right, and forward motor populations select corresponding MBON sectors.
5. PAM cells receive a small MBON-derived input and provide the dopamine population used by the learning mechanism.

Weights are initialized deterministically. The feed-forward path is sparse and fixed-width, keeping memory access predictable.

## LIF update

Each neural tick performs:

1. Encode the current visual features into sensory current.
2. Accumulate sparse incoming spikes.
3. Apply exponential membrane leak.
4. Generate threshold spikes unless refractory.
5. Reset spiking membranes and update refractory counters.
6. Update activity traces.
7. Update edge eligibility from recent pre/post activity.
8. Apply dopamine-gated plasticity when dopamine is active.
9. Clear transient input current.

Rayon is used for independent array passes. The project deliberately limits the configurable worker count to 1–4 for the target dual-core/quad-thread machine.

## Learning

A food collision calls `Brain::inject_dopamine(1.0)`. This is a real mutation of the stored weight array, not a hardcoded behavior switch.

Eligibility is strongest when recent source and target activity coincide. Dopamine scales the eligible update, and every weight is clamped to `[-1, 1]`. The brain exposes changed-synapse and mean-weight telemetry plus a deterministic checksum.

The headless training protocol repeatedly:

1. resets only the fly/episode state, preserving learned weights;
2. places food at a deterministic cue position;
3. runs a fixed cue-exposure interval;
4. records closest approach;
5. delivers reward by moving food onto the fly and allowing the normal collision path to fire;
6. starts the next episode with the learned brain.

This makes training reproducible while keeping the reward pathway inside the simulator.

## Physics

The fly is a lightweight planar body with:

- position and velocity
- yaw
- forward vector
- thrust from motor activity
- exponential drag
- maximum speed
- arena wall response
- circular food collision

Food respawn uses a deterministic xorshift state. No general-purpose physics engine is used.

## Camera and renderer

`camera.rs` contains hand-written column-major matrices for translation, scale, Y/Z rotation, look-at, and perspective projection. `CameraRig` follows the fly with exponential smoothing and a small forward look-ahead.

`render.rs` uses `wgpu 0.20` and `winit 0.30` with:

- one render pipeline
- one dynamic uniform buffer
- one depth attachment
- hard-coded cube/food geometry
- low-power adapter preference
- Vulkan or OpenGL backend selection through wgpu

The fly is assembled from a few scaled cube primitives: abdomen, thorax, head, and two thin wings. This avoids model loading and texture memory.

## CLI and diagnostics

`main.rs` uses winit's `ApplicationHandler` API. CPU-only modes never initialize the GPU.

- `--headless`: fixed deterministic CPU smoke run
- `--benchmark [seconds]`: neural throughput and memory telemetry
- `--train [seconds]`: deterministic reward-learning experiment
- `--threads N`: 1–4 Rayon workers

Benchmark metrics include neurons/sec, synapses/sec, average step time, core-brain memory, process RSS on Linux, reward count, and weight checksum.

## Determinism

Determinism is maintained by:

- deterministic topology generation
- deterministic initial weights
- fixed simulation timestep
- deterministic xorshift food respawn
- no wall-clock state in the CPU simulation
- deterministic training cue sequence

Tests compare positions, yaw, rewards, and weight checksums between identical simulations.

## Performance constraints

The primary target is an i5-3210M with Intel HD 4000 graphics. Wisp therefore favors:

- contiguous arrays
- sparse fixed-width connectivity
- tiny geometry
- no textures
- no post-processing
- no shadows or reflections
- no ECS
- no per-neuron heap allocations
- bounded Rayon parallelism

The benchmark is the authority for actual throughput on a given machine.

## Deliberate limitations

The simulator does not claim biological completeness. It omits the full fly connectome, detailed neuron classes, dendritic morphology, realistic olfactory processing, retinal rendering, muscle models, wing aerodynamics, and full biological neuromodulation. Those omissions are intentional so the experiment remains small enough to inspect, test, benchmark, and run on old integrated graphics hardware.
