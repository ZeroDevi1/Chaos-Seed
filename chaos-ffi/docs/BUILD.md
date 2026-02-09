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

另外，本仓库提供了一个 Rust 内置生成器（不依赖系统安装 `cbindgen` 可执行文件），用于生成：
- `chaos-ffi/include/chaos_ffi_bindings.h`

生成命令：

```bash
cargo run -p chaos-ffi --features gen-header --bin gen_header
```

说明：
- `chaos_ffi.h` 是稳定 wrapper（含 `extern "C"`），内部 `#include "chaos_ffi_bindings.h"`。
- 当你新增/修改 FFI 导出函数后，运行上述命令即可更新 bindings 头文件。

## 真实站点集成测试（可选）

默认 `cargo test -p chaos-ffi` 不会发起网络请求。

若你需要在本机对“真实 URL”做一次解析校验，可以运行：

```bash
cargo test -p chaos-ffi --features live-tests --test livestream_live -- \
  --bili-url <URL> --huya-url <URL>
```

说明：
- 这些 URL **不会**写进仓库，必须在终端运行时传入（避免在代码中出现真实信息）。
- 你也可以只传一个 URL；未传入的部分会被跳过。
- 若你想输出解析后的完整 JSON，追加参数：`--dump-json`。
