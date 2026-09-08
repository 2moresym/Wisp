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
    +-- PEB / TEB / TLS / module graph
    |
    +-- D3D9/10/11 -> DXVK -> Vulkan
    +-- D3D12      -> VKD3D-Proton -> Vulkan
    +-- Vulkan     -> native Vulkan
    +-- OpenGL/WGL -> EGL/GLX -> Mesa
    +-- XInput     -> evdev/SDL
    +-- XAudio2    -> PipeWire/ALSA
```

## Workspace

- `wisp-core` — handles, process/thread environment, synchronization, module lifecycle, TLS, and case-insensitive VFS primitives.
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

## Current runtime foundation

The core now has explicit PEB/TEB/TLS state, Linux x86-64 GS-backed TEB installation, process-local module registration with dependency-first ordering and export lookup, a directory-aware case-insensitive VFS cache, and a checked loader-phase state machine.

These components are foundations rather than claims of complete Win32 compatibility. Real PE import execution, CRT initialization, TLS callback execution, DLL entry-point invocation, and graphics translation remain subsequent milestones.

## Phase 2

See [`docs/PHASE2.md`](docs/PHASE2.md). Anti-cheat is explicitly deferred until the ordinary game compatibility layer is mature.

## Status

Early runtime foundation with native window backends and x86-64 thread environment support. The next major milestone is wiring real PE module loading/import resolution and Windows loader semantics into the runtime.
