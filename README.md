# Wisp

Wisp is a lightweight Rust research sandbox for a Drosophila-inspired neural sub-graph coupled to a tiny hardware-accelerated 3D arena.

> Wisp is an engineering emulator, not a complete biological reconstruction of the fruit-fly brain.

## Design targets

- ~5,000 simulated LIF neurons rather than the full ~166k-neuron fly brain.
- Flat `Vec` storage for membrane state, spikes, traces and sparse synapses.
- Sparse CSR-style incoming connectivity to keep memory and cache traffic small.
- Rayon parallelism across independent neuron updates.
- Dopamine-gated Hebbian plasticity representing a simplified PAM/Mushroom Body reward pathway.
- `wgpu` + `winit` native windowing with Vulkan/OpenGL backends.
- No Electron, browser runtime, ECS dependency or heavyweight scene framework.
- Small low-poly renderer intended for Intel HD 4000-class hardware.

## Architecture

```text
src/
├── main.rs       fixed-timestep brain + render/event loop
├── brain.rs      5k-neuron LIF model, sparse graph, dopamine plasticity
├── physics.rs    fly/food state, collision and visual feature extraction
└── render.rs     minimal wgpu pipeline and GPU buffers
```

The fixed neural timestep is 2 ms while rendering is event-driven. Visual features are generated from the fly-to-food relative angle/distance, encoded into the visual population, simulated, then mapped directly into turning/forward motor forces.

## Build

```bash
cargo build --release
cargo run --release
```

## Next milestones

1. Replace the initial placeholder projection with a true tiny 3D camera and arena mesh.
2. Add explicit visual → Kenyon → MBON/PAM → motor populations.
3. Add measurable STDP eligibility traces and weight histograms.
4. Add deterministic benchmark mode and memory/CPU telemetry.
5. Add GPU capability-based renderer fallback and Intel HD 4000 tuning.
