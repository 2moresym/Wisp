# Wisp

Wisp is a deliberately small Rust research platform coupling a reduced Drosophila-inspired neural subgraph to a hardware-accelerated low-poly 3D fly arena.

> Wisp is a computational emulator and research sandbox, not a complete reconstruction of the ~166,000-neuron Drosophila brain.

## Design goals

- 5,000 compact leaky integrate-and-fire neurons.
- Flat structure-of-arrays state in contiguous `Vec` storage.
- 60,000 sparse incoming synapses using `u16` source indices and `f32` weights.
- Visual -> Kenyon -> MBON -> motor pathway plus a PAM-like dopamine population.
- Eligibility traces and reward-modulated Hebbian plasticity.
- Fixed 2 ms neural timestep independent of rendering.
- Deterministic topology, simulation, food respawn, training protocol, and weight checksum.
- Native `winit` 0.30 + `wgpu` 0.20 with no ECS or game engine.
- Low-polygon geometry and tiny GPU buffers suitable for Intel HD 4000-class hardware.

## Layout

```text
src/
├── main.rs       CLI, ApplicationHandler event loop, fixed-step scheduler, diagnostics
├── brain.rs      5k-neuron SoA model, sparse graph, LIF dynamics, dopamine learning
├── sim.rs        deterministic brain + physics orchestration and training helpers
├── physics.rs    fly dynamics, food collision, sensory feature encoding
├── camera.rs     hand-written matrices and smoothed follow camera
├── render.rs     lightweight wgpu pipeline and low-poly arena/fly
└── lib.rs        reusable CPU-side modules for tests
```

## Neural model

The default populations are:

```text
VISION   1024
   ↓
KENYON   2560
   ↓
MBON      512
   ↓
MOTOR     256

PAM        32  ── dopamine gate ──> plasticity
```

The graph uses twelve incoming edges per neuron. Visual sectors are preserved through the Kenyon and MBON layers so that left/right visual activity has a meaningful relationship with left/right motor populations. The motor snapshot converts activity into turn and forward commands.

The sensory abstraction contains:

- relative food angle
- distance/proximity
- lateral bearing
- approach velocity
- food presence through the active visual sector

No rendered image is fed into the brain and no computer-vision stack is required.

## Dopamine learning

A food collision injects dopamine through the PAM population. Recent pre/post activity creates edge eligibility, and dopamine gates actual changes to stored synaptic weights. Weights are clamped to `[-1, 1]`.

Telemetry includes reward count, dopamine level, changed synapses, mean changed weight before/after, and a deterministic FNV-style weight checksum.

The learning loop is therefore:

```text
see food
  -> visual sector activity
  -> Kenyon / MBON activity
  -> motor output
  -> fly motion
  -> food collision
  -> dopamine
  -> eligibility-weighted plasticity
  -> changed future responses
```

## 3D arena

The renderer intentionally stays small:

- floor and four walls
- low-poly food marker
- recognizable low-poly fly with head, thorax, abdomen, and two wings
- depth buffering
- one simple shader/pipeline
- smoothed camera follow with velocity-aligned look-ahead

There are no textures, shadows, PBR, bloom, SSAO, reflections, ray tracing, or GPU readbacks. The fly remains only a handful of cube draws and a food marker.

## CLI

Build with:

```bash
cargo build --release
```

Run the interactive arena:

```bash
cargo run --release
```

CPU-only deterministic check:

```bash
cargo run --release -- --headless
```

Benchmark the neural engine:

```bash
cargo run --release -- --benchmark 5 --threads 1
cargo run --release -- --benchmark 5 --threads 2
cargo run --release -- --benchmark 5 --threads 4
```

Benchmark output reports neurons, synapses, steps, elapsed time, steps/sec, neurons/sec, synapses/sec, average step time, core-brain memory, process RSS when available, rewards, and checksum.

Deterministic reward-learning training:

```bash
cargo run --release -- --train 60 --threads 2
```

Training presents the same deterministic food cue, measures the fly's closest approach, then delivers reward through a real collision at the end of the cue exposure. The next episode reuses the learned brain, allowing behavioral change to be measured without hardcoded motor behavior.

`--threads` is clamped to 1..4 because Wisp targets a dual-core/quad-thread i5-3210M-class CPU.

## Testing and CI

```bash
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
cargo build --release
cargo run --release -- --headless
cargo run --release -- --benchmark 5
```

CPU-side unit and integration tests do not require a GPU window. CI repeats formatting, checking, tests, and release builds.

## Memory and performance philosophy

The brain's explicitly accounted arrays are around 1.0 MiB for the default 5,000-neuron/60,000-edge graph, comfortably below the 1–5 MiB core-brain target and far below 50 MiB. Runtime allocator and renderer overhead are separate from this core estimate.

The hot path avoids per-neuron allocations, boxed neurons, ECS overhead, repeated topology generation, and GPU readbacks. Rayon is used for independent dense state passes and can be restricted to a small worker count. Benchmark both serial and parallel configurations on the target machine rather than assuming a speedup.

## Limitations

Wisp does not reproduce the full Drosophila connectome, detailed neuron morphologies, real sensory transduction, realistic flight aerodynamics, or biological timing. The visual sectors, Kenyon cells, MBONs, motor populations, and dopamine pathway are intentionally simplified computational analogues designed for deterministic experimentation on low-end hardware.
