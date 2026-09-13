# Wisp Architecture

Wisp is a deliberately small Rust research platform combining a reduced Drosophila-inspired neural subgraph with a low-poly 3D arena.

## Core targets

- 5,000 LIF neurons.
- Flat structure-of-arrays state.
- Sparse incoming CSR-style synapses.
- Explicit visual -> mushroom body -> motor pathway.
- A small PAM-like dopamine population for reward modulation.
- Fixed 2 ms neural timestep.
- Native winit + wgpu renderer.
- No ECS, browser runtime, or heavyweight game framework.
- Practical operation on Intel HD 4000-class hardware.

## Module boundaries

- `brain.rs`: neural state, topology, LIF stepping, eligibility traces, dopamine plasticity.
- `physics.rs`: fly/food dynamics, sensory encoding and collision.
- `sim.rs`: deterministic fixed-step orchestration between brain and physics.
- `camera.rs`: tiny CPU-side perspective camera and matrix math.
- `render.rs`: one lightweight wgpu pipeline and low-poly arena primitives.
- `main.rs`: event loop, CLI flags, fixed-step scheduling and diagnostics.

The emulator is a computational abstraction, not a reconstruction of the complete ~166k-neuron Drosophila brain.
