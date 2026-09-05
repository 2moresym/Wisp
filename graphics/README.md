# Graphics backends

Wisp keeps graphics translation outside the Rust loader hot path.

```text
D3D9/10/11 -> DXVK -> Vulkan
D3D12      -> VKD3D-Proton -> Vulkan
Vulkan     -> native Vulkan
OpenGL/WGL -> EGL/GLX -> Mesa
```

Vendor DXVK and VKD3D-Proton under `graphics/` as pinned revisions during the integration milestone. They are build-time/native dependencies, not Rust reimplementations.
