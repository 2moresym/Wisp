# Wisp PE fixture

A tiny x86-64 Windows PE32+ executable used to exercise Wisp's loader.

The fixture intentionally starts with no imports and no CRT dependency. Its entry point is `wisp_entry`, which returns `42`.

## Build

On Debian/Ubuntu with MinGW-w64 installed:

```bash
sudo apt install gcc-mingw-w64-x86-64 cmake
cmake -S tests/pe-fixture -B tests/pe-fixture/build -DCMAKE_BUILD_TYPE=Release
cmake --build tests/pe-fixture/build
```

The resulting executable is:

```text
tests/pe-fixture/build/wisp-test.exe
```

## Intended test progression

1. Parse PE32+ headers and sections.
2. Map the image into Wisp memory.
3. Apply relocations when the preferred base is unavailable.
4. Enforce section permissions.
5. Resolve imports once imports are introduced into a later fixture.
6. Transfer control to the entry point and verify the return value is `42`.
