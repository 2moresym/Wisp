# Wisp Continuation Prompt

We are continuing development of the GitHub repository:

`https://github.com/2moresym/Wisp`

You are the primary implementation agent. Work **directly on the GitHub repository** and actually modify, test, commit, and push code. Do not just give me suggested code or an architecture essay.

## PROJECT

Wisp is a lightweight Rust research platform combining:

1. A reduced Drosophila-inspired neural brain emulator.
2. A hardware-accelerated low-poly 3D arena.
3. A simulated fly whose behavior is controlled by the neural system.
4. A dopamine/reward learning mechanism that changes synaptic weights.

Target hardware:

- Intel Core i5-3210M
- 2 physical cores / 4 logical threads
- 3 MB L3 cache
- Intel HD Graphics 4000
- Linux
- Bore CPU scheduler

The project must prioritize low memory use, cache locality, low CPU overhead, minimal GPU workload, deterministic simulation, and simple Rust architecture.

## IMPORTANT: CURRENT STATE

The repository has already reached a successful GitHub Actions release build after fixing the initial Rust and wgpu API errors.

Current Cargo dependency versions include:

```toml
rayon = "1.10"
wgpu = "0.20"
winit = "0.30"
bytemuck = { version = "1.16", features = ["derive"] }
pollster = "0.3"
```

Do NOT blindly upgrade wgpu or winit just because newer APIs are familiar. Respect the versions actually present in `Cargo.toml`.

Current repository structure is approximately:

```text
Cargo.toml
README.md
ARCHITECTURE.md
WISP_CONTINUATION_PROMPT.md
.github/workflows/ci.yml
src/
├── brain.rs
├── camera.rs
├── lib.rs
├── main.rs
├── physics.rs
├── render.rs
└── sim.rs
```

## FIRST ACTION

Before touching anything:

```bash
git status
git branch --show-current
git log --oneline -15
```

Then inspect the **entire repository**, including all current source files, README, architecture documentation, Cargo.toml, CI, and recent commits.

The repository may change while you work, so always inspect the live current state before replacing files.

Preserve good existing work.

---

# CURRENT IMPLEMENTATION

The neural subsystem currently targets approximately:

```text
5,000 neurons
```

with explicit population bands approximately like:

```text
VISION
KENYON / MUSHROOM BODY
MBON
MOTOR LEFT
MOTOR RIGHT
MOTOR FORWARD
PAM
REST
```

The brain uses:

- flat vector/state-array storage
- sparse connectivity
- CSR-style incoming synapses
- u16 source indices
- f32 weights
- f32 eligibility traces
- refractory state
- LIF dynamics
- Rayon parallelism
- deterministic topology
- dopamine-gated plasticity
- weight clamping
- weight checksum

The intent is a computational abstraction, **not a biologically complete 166,000-neuron Drosophila reconstruction**.

The simulation currently includes:

- Fly
- Food
- Position
- Velocity
- Rotation/yaw
- Fly→food visual encoding
- motor-driven motion
- collision detection
- dopamine reward
- deterministic food respawn
- reward cooldown

The renderer currently has:

- winit window
- wgpu rendering
- perspective camera
- depth buffer
- low-poly arena
- low-poly fly representation
- low-poly food representation
- single lightweight graphics pipeline
- dynamic uniform buffer slots
- Vulkan/OpenGL-compatible wgpu instance configuration

The executable currently has:

```text
--headless
--benchmark [seconds]
--threads N
```

and a fixed 2 ms neural timestep.

---

# YOUR JOB NOW

Do NOT stop at the current successful compilation.

Take Wisp toward a genuinely usable simulation.

## 1. FIX ALL WARNINGS

Clean up the current warning set where practical.

Known warnings include:

- deprecated `winit::EventLoop::create_window`
- deprecated `winit::EventLoop::run`
- unused `MOTOR_RIGHT_COUNT`
- unused `REST_START`
- unused `REST_COUNT`
- unused brain telemetry methods
- unused `reset_near_origin`
- unused `force_food_at_fly`

Do not silence warnings with blanket `allow(dead_code)` unless there is a strong architectural reason.

Prefer:

- actually using useful APIs
- removing dead APIs
- migrating deprecated winit APIs safely

Keep compatibility with the currently pinned versions.

---

# 2. MAKE THE FLY ACTUALLY LOOK LIKE A FLY

The current fly is represented with a primitive.

Upgrade it to a tiny recognizable low-poly insect.

Keep it extremely cheap.

A good design would be:

```text
body
thorax
abdomen
head
two wings
```

Use simple hard-coded geometry.

Do NOT introduce a model-loading pipeline.

Do NOT add textures unless necessary.

The fly should remain only a few dozen triangles.

Its orientation must follow `rotation_y`.

Movement must continue to come from the neural motor output.

---

# 3. IMPROVE THE CAMERA

Keep the current hand-written lightweight matrix math.

Implement a proper:

```text
Camera
View matrix
Projection matrix
Look-at
Perspective
```

Then make the camera follow the fly smoothly.

Avoid adding glam unless profiling proves it worthwhile.

The camera must remain CPU-cheap.

Consider:

```text
camera lag / smoothing
distance
height
look-ahead based on velocity
```

but keep it simple.

---

# 4. IMPROVE THE ARENA

Make the arena visually useful for experiments.

Add cheap geometry such as:

```text
floor
walls
simple food marker
optional grid/reference markings
```

No expensive post-processing.

No shadows.

No PBR.

No bloom.

No SSAO.

No reflections.

No large textures.

The Intel HD 4000 must remain the primary rendering constraint.

---

# 5. MAKE VISUAL INPUT MORE INTERESTING

The current sensory system derives information directly from fly→food geometry.

Expand it slightly.

The brain should receive something like:

```text
distance
relative angle
left/right position
approach velocity
food visibility/presence
```

Still do NOT render the scene into an image and perform computer vision.

The visual input should remain a lightweight simulation-space abstraction.

---

# 6. IMPROVE BRAIN → BEHAVIOR

This is extremely important.

The current system must evolve beyond arbitrary random motor behavior.

Design the feed-forward topology so that visual sectors have meaningful relationships with motor sectors.

For example:

```text
food left
    ↓
left/right motor bias
    ↓
turn toward food
```

The fly should be capable of developing an attraction behavior through reward.

We want a demonstrable loop:

```text
see food
→ approach
→ collide
→ dopamine
→ synaptic strengthening
→ behavior changes
```

The behavior does not need to be biologically exact.

It needs to be computationally meaningful and measurable.

---

# 7. IMPROVE DOPAMINE LEARNING

The current dopamine pathway must remain real.

Do NOT fake learning through hardcoded behavior.

Dopamine must modify actual stored synaptic weights.

Improve the learning mechanism so that:

- eligibility traces are meaningful
- reward reinforces recently active pathways
- weights stay clamped
- repeated rewards produce measurable behavioral change
- learning is deterministic
- reward does not spam terminal output

Provide useful statistics such as:

```text
dopamine level
reward count
changed synapses
mean changed weight
weight checksum
```

---

# 8. ADD EXPERIMENT / TRAINING MODE

Add a proper headless training mode.

For example:

```bash
cargo run --release -- --train 60
```

or an equivalent lightweight CLI.

Training mode should:

- run the simulation without opening a window
- repeatedly expose the fly to reward opportunities
- report progress
- measure whether behavior is changing
- finish with reproducible statistics

Example:

```text
Wisp training
episode=1 reward=1 distance=...
episode=2 reward=1 distance=...
...
learning improvement: ...
final checksum: ...
```

Keep it deterministic unless a seed is explicitly changed.

---

# 9. IMPROVE BENCHMARK MODE

The current benchmark should become useful for profiling the neural engine.

Measure:

```text
neurons
synapses
steps
elapsed time
steps/sec
neurons/sec
synapses/sec
average step time
memory usage
rewards
checksum
```

Add a way to compare thread counts if practical:

```text
--benchmark 5 --threads 1
--benchmark 5 --threads 4
```

Do not claim speedups without actually measuring them.

---

# 10. MEMORY ACCOUNTING

Ensure the neural simulation remains far below:

```text
50 MB
```

Prefer around:

```text
1–5 MB
```

for the core brain data.

Document actual estimated memory.

Keep arrays contiguous.

Avoid:

```rust
Vec<Box<Neuron>>
```

and per-neuron allocations.

Keep the hot path data-oriented.

---

# 11. NEURAL HOT LOOP OPTIMIZATION

Profile the architecture conceptually and fix obvious inefficiencies.

Important goals:

- avoid repeated allocations
- avoid unnecessary clones
- avoid repeated vector initialization
- avoid redundant calculations
- minimize synchronization
- preserve sequential memory access
- use Rayon only where work is large enough to justify it

Do not blindly use Rayon everywhere.

The target CPU has only 2 physical cores.

Benchmark serial vs 4-thread execution.

---

# 12. DETERMINISM

Provide deterministic behavior.

Two simulations created with the same configuration should produce identical:

```text
positions
rewards
weights
checksum
```

after the same number of steps.

Add tests.

---

# 13. TEST SUITE

Expand tests for:

### Brain
- topology size
- deterministic topology
- LIF threshold
- refractory behavior
- spikes
- dopamine
- weight changes
- weight clamp
- deterministic checksum

### Physics
- movement
- turning
- collision
- arena bounds
- angle normalization
- visual encoding

### Simulation
- deterministic simulation
- reward event
- food respawn
- training progression

### Camera
- perspective matrix
- look-at matrix
- camera following

GPU initialization should not be required for normal unit tests.

---

# 14. CI

Keep the GitHub Actions CI authoritative.

It should perform:

```text
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
cargo build --release
```

Fix any failures that appear.

Do not leave CI red.

---

# 15. DOCUMENTATION

Update:

```text
README.md
ARCHITECTURE.md
```

to reflect the actual implementation.

Document:

- architecture
- brain model
- populations
- dopamine mechanism
- physics
- visual encoding
- rendering
- CLI
- benchmark
- training mode
- memory budget
- performance philosophy
- limitations

Be explicit that this is a computational emulator.

---

# 16. CODE STYLE

Use idiomatic Rust.

Prefer readable code over ultra-compressed one-line code.

The previous implementation became overly compressed in places.

Do NOT continue that pattern.

Write maintainable Rust like:

```rust
let visual = self.fly.visual_input(&self.food);
let snapshot = self.brain.step(visual, NEURAL_DT);

self.fly
    .apply_motor(snapshot.turn, snapshot.forward, NEURAL_DT);

self.fly.integrate(NEURAL_DT);
```

instead of massive one-line expressions.

---

# 17. GIT WORKFLOW

Create logical commits.

Preferred progression:

```text
fix: clean compiler warnings
feat: improve low-poly fly model
feat: improve camera following
feat: expand arena and sensory model
feat: improve reward-guided behavior
feat: add deterministic training mode
perf: improve neural benchmark
test: expand deterministic regression coverage
docs: update Wisp architecture
```

Do not make fake commits.

Every commit should contain real work.

Push commits to GitHub.

Do not force-push or rewrite history unnecessarily.

---

# 18. VALIDATION

After implementation:

```bash
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
cargo build --release
cargo run --release -- --headless
cargo run --release -- --benchmark 5
```

Run whatever subset is practical in CI/environment.

If GPU execution isn't available in the environment, clearly distinguish:

```text
CPU/unit validation
GPU compile validation
GPU runtime validation
```

Do not pretend a renderer has been runtime tested when it hasn't.

---

# 19. FINAL STANDARD

Do not stop after fixing the first compiler error.

Keep iterating until:

```text
cargo fmt --check     ✅
cargo check           ✅
cargo test            ✅
cargo build --release ✅
CI                    ✅
```

and the repository contains a coherent experimental fly-brain simulation rather than a collection of placeholders.

At the end report:

```text
Wisp implementation status

Current commit:
Major systems completed:
Tests:
CI:
Benchmark:
Training mode:
Memory estimate:
Known limitations:
Next highest-value improvement:
```

MOST IMPORTANT:

**Actually edit the repository.**
**Actually test it.**
**Actually commit it.**
**Actually push it.**

Do not merely tell me how to build Wisp.
