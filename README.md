# Wisp

Wisp is a deliberately small Rust research platform that couples a Drosophila-inspired neural subgraph to a hardware-accelerated 3D fly arena.

> Wisp is a computational emulator and research sandbox, not a complete reconstruction of the ~166,000-neuron Drosophila brain.

## Goals

- 5,000 compact LIF neurons instead of a full-brain reconstruction.
- Structure-of-arrays state in contiguous `Vec` storage.
- Sparse 12-edge-per-neuron layered connectivity.
- Visual -> Kenyon -> MBON -> motor pathway plus a PAM-like dopamine population.
- Eligibility traces and reward-modulated Hebbian plasticity.
- Fixed 2 ms neural timestep independent of rendering.
- Native `winit` + `wgpu`, with Vulkan/OpenGL backends and no ECS/game engine.
- Low-polygon geometry and tiny GPU buffers suitable for Intel HD 4000-class hardware.

## Layout

```text
src/
├── main.rs       CLI, fixed-step scheduler, native window/event loop
├── brain.rs      5k-neuron SoA model, sparse graph, dopamine learning
├── sim.rs        deterministic brain + physics orchestration
├── physics.rs    fly dynamics, food collision, visual feature encoding
├── camera.rs     small CPU-side view/projection math
├── render.rs     low-poly wgpu renderer and depth buffer
└── lib.rs        reusable CPU-side modules for tests
```

## Neural model

The simulator stores membrane potential, spike state, refractory counters, sensory current, traces, synaptic weights, and per-edge eligibility traces in flat arrays. Connectivity is layered rather than random across the whole graph:

```text
VISION 1024
   ↓
KENYON 2560
   ↓
MBON 512
   ↓
MOTOR 256

PAM 32  ── dopamine gate ──> plasticity
```

The neuron model is a compact leaky integrate-and-fire update. The learning path uses an eligibility signal from recent pre/post activity and a dopamine-gated Hebbian potentiation term. Weights are clamped to `[-1, 1]`.

The neural representation is intentionally simplified. It does not claim to reproduce all Drosophila cell types, connectome details, neuromodulators, or sensory processing.

## Simulation and arena

The fly tracks position, velocity, and yaw. Food position is converted each neural tick into relative distance, angle, and lateral bearing. Motor population activity becomes turning and forward thrust. A collision injects dopamine, records a reward event, and deterministically respawns the food.

The renderer uses a single pipeline, a depth attachment, a handful of low-poly draws, and dynamic object uniforms. The camera follows behind the fly so the effect of the brain-controlled movement is visible.

## Performance envelope

The default graph has 5,000 neurons and 60,000 synapses. CPU-side neural storage is intentionally well below the 10–50 MB target. Rayon parallelizes independent state passes and can be restricted to 1–4 worker threads for old CPUs.

The code avoids per-neuron heap allocations, ECS overhead, texture-heavy materials, post-processing, shadows, ray tracing, and GPU readbacks.

## Build

```bash
cargo build --release
cargo run --release
```

Useful modes:

```bash
cargo run --release -- --help
cargo run --release -- --headless
cargo run --release -- --benchmark 5
cargo run --release -- --benchmark 5 --threads 2
cargo run --release -- --benchmark 5 --threads 4
```

`--headless` exercises the CPU simulation without opening a window. `--benchmark` reports neural steps/second, step time, memory estimate, rewards, and a deterministic weight checksum.

## Testing

```bash
cargo check --all-targets
cargo test --all-targets
cargo build --release
```

The CPU-side tests do not require a GPU window. GitHub Actions repeats the check, test, and release-build steps on every push and pull request.

## Hardware target

Wisp is tuned for the constraints of an old dual-core/quad-thread mobile CPU and Intel HD 4000-class integrated graphics. Actual frame rate and neural throughput depend on the installed Mesa/wgpu backend, display resolution, and OS configuration; benchmark your own machine rather than treating the target as a guaranteed FPS number.
