# Wisp

A minimal-overhead Windows game compatibility layer for Linux, built from scratch in Rust with a deliberately tiny C/C++ graphics boundary.

Wisp is **not a Wine clone**. The target is x86-64 games, not general Win32 compatibility.

## Architecture

```text
Windows game
    |
    v
Wisp ABI / PE loader / NT runtime
    |
    +-- memory + threads + handles + sync + VFS
    |
    +-- D3D9/10/11 -> DXVK -> Vulkan
    +-- D3D12      -> VKD3D-Proton -> Vulkan
    +-- Vulkan     -> native Vulkan
    +-- OpenGL/WGL -> EGL/GLX -> Mesa
    +-- XInput     -> evdev/SDL
    +-- XAudio2    -> PipeWire/ALSA
```

## Workspace

- `wisp-core` — handles, synchronization and case-insensitive VFS primitives.
- `wisp-pe-loader` — x86-64 PE validation and explicit loader state machine foundation.
- `wisp-syscall` — direct Linux-backed NT memory/thread ABI layer.
- `wisp-loader-cli` — initial `wisp game.exe` inspection/loader entry point.
- `wisp-glue-c` — intentionally tiny native C ABI boundary.

## Performance rules

- No general Win32 surface.
- No 32-bit/WoW64 in the initial target.
- No allocations on synchronization hot paths.
- Bounded userspace spin before futex blocking.
- Direct native Vulkan/OpenGL paths.
- Rust/C crossings only at stable ownership/ABI boundaries.
- Loader work happens once; frame loops stay out of the loader.

## Phase 2

See [`docs/PHASE2.md`](docs/PHASE2.md). Anti-cheat is explicitly deferred until the ordinary game compatibility layer is mature.

## Status

Early skeleton. The next implementation milestone is real PE mapping/relocations/import resolution followed by a minimal Win32 process/thread environment and graphics surface bridge.
