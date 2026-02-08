# Build & Outputs

## Windows (MSVC)

From repo root:

```powershell
cargo build -p chaos-ffi --release
```

Outputs:
- `target\release\chaos_ffi.dll`
- `target\release\chaos_ffi.lib` (import library, may be generated depending on toolchain)

Deploy by placing `chaos_ffi.dll` next to your `.exe` (WinUI3/Console/Qt app).

## Linux

From repo root:

```bash
cargo build -p chaos-ffi --release
```

Outputs:
- `target/release/libchaos_ffi.so`

Deploy by placing `libchaos_ffi.so` next to your executable or in a loader-visible path.

## Header

A hand-written header is provided:
- `chaos-ffi/include/chaos_ffi.h`

You may generate one with `cbindgen` for verification, but it is not required.

