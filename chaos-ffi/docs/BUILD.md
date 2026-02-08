# 构建与产物

## Windows (MSVC)

在仓库根目录执行：

```powershell
cargo build -p chaos-ffi --release
```

产物：
- `target\release\chaos_ffi.dll`
- `target\release\chaos_ffi.lib`（import library；是否生成取决于工具链配置）

部署方式：将 `chaos_ffi.dll` 放到你的 `.exe` 同目录（WinUI3/Console/Qt 应用）。

## Linux

在仓库根目录执行：

```bash
cargo build -p chaos-ffi --release
```

产物：
- `target/release/libchaos_ffi.so`

部署方式：将 `libchaos_ffi.so` 放到可执行文件同目录，或放到动态链接器可搜索路径中。

## Header

仓库内提供了一份手写头文件：
- `chaos-ffi/include/chaos_ffi.h`

你也可以用 `cbindgen` 生成头文件做校验，但这不是必需步骤。
