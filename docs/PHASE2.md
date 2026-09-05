# Wisp Phase 2

Phase 2 is the game-focused runtime surface. It is intentionally narrow and does not attempt to reproduce general Win32.

## Graphics

- D3D9/10/11: DXVK is vendored as a build dependency and receives the Wisp window/Vulkan surface bridge.
- D3D12: VKD3D-Proton is vendored as a build dependency.
- Vulkan: native Vulkan loader path; no translation.
- OpenGL/WGL: native EGL/GLX/Mesa path; no GL-to-Vulkan translation.

The Rust ABI layer owns process/thread/memory/window lifetime. Graphics implementations remain native code so their hot loops do not cross Rust/C unnecessarily.

## Synchronization

`WaitOnAddress`/SRW locks/critical sections use a userspace atomic fast path, a short bounded spin phase, then Linux private futex wait/wake. The hot path performs no allocation.

Spin counts are per primitive and tunable from profiling. The kernel is entered only after the value is still unchanged after spinning.

## I/O completion

The intended IOCP backend is an epoll/`io_uring` adapter. Readiness and completion are normalized into a compact Wisp completion record; allocations are pooled outside the hot completion path.

## Input

XInput/DirectInput map to evdev/SDL-compatible controller data. Raw device handling stays out of the PE/NT core.

## Audio

XAudio2/DirectSound map to PipeWire first, with ALSA as the low-level fallback. Audio callbacks must not enter the general object manager.

## Case-insensitive filesystem

Linux remains case-sensitive. Wisp lazily scans each accessed directory and maintains a lowercase-name -> real-name map. Lookup first probes the exact Linux name, then the cached folded name. Directory caches are invalidated on writes/renames.

## Anti-cheat

Explicitly deferred. No anti-cheat compatibility code is part of Phase 2.
