# Livestream（直播源解析）— chaos-ffi（中文）

本页专门说明 `chaos-ffi` 暴露的直播解析接口（虎牙 / 斗鱼 / 哔哩哔哩直播 BiliLive），以及为什么需要 “二段解析（resolve variant）”。

## 1. 设计目标

- **ABI 稳定**：对外只暴露 C ABI，数据统一用 JSON 传递。
- **与 UI 解耦**：core 只负责解析直播源；UI 自己决定怎么展示清晰度/线路、怎么播放。
- **可扩展**：未来增加平台只需要在 `chaos-core` 扩展平台模块，FFI 仍然沿用同一套 JSON 结构。

## 2. 关键结构

### LiveManifest

一次 `decode_manifest` 返回一个 `LiveManifest`：
- `site`：平台（`BiliLive` / `Douyu` / `Huya`）
- `room_id`：canonical room id（例如 BiliLive 会从短号解析成长号）
- `info`：标题/主播头像/封面/是否开播（best-effort）
- `playback`：播放提示（例如 `referer` / `user_agent`，供播放器设置）
- `variants`：清晰度/线路列表（每个元素是 `StreamVariant`）

### StreamVariant

- `id`：稳定 id，用于二段解析（例如 `bili_live:2000:原画`）
- `label`：展示名（原画/蓝光/高清…）
- `quality`：排序用数值（BiliLive=qn；Huya=bitrate；Douyu=bit）
- `rate`：斗鱼专用字段（用于二次请求补齐 URL）
- `url` / `backup_urls`：最终可播放地址及备选地址

## 3. 为什么需要二段解析（resolve variant）

部分平台的接口行为是：
- 第一次请求只返回“当前默认清晰度”的可播放 URL
- 其它清晰度只给一个标识（比如 `rate/qn`），需要带着这个标识再请求一次才会返回 URL

因此在 FFI 层提供：
- `decode_manifest`：拿到 manifest + variants（可能部分 variant 没有 url）
- `resolve_variant`：根据 `variant_id` 补齐特定清晰度的 url/backup_urls

这样 UI 可以先快速展示清晰度列表，再在用户切换清晰度时按需补全 URL。

## 4. 典型调用流程（伪代码）

1) 解析 manifest：

```c
char* s = chaos_livestream_decode_manifest_json("<BILILIVE_URL>", 1);
// s 是 LiveManifest JSON，解析后拿到 variants 列表
```

2) 用户选择清晰度（拿到 `variant_id`），补齐 URL：

```c
char* v = chaos_livestream_resolve_variant_json("<BILILIVE_URL>", "bili_live:2000:原画");
// v 是 StreamVariant JSON，包含 url + backup_urls
```

3) 播放器侧建议：
- 使用 `manifest.playback.referer` / `manifest.playback.user_agent` 作为请求头配置
- 优先使用 `variant.url`；失败时依次尝试 `backup_urls`

## 5. 常见问题

### 5.1 返回 NULL 怎么排查？

任何失败都会返回 `NULL`。请立即调用：

```c
char* err = chaos_ffi_last_error_json();
```

拿到 JSON 错误信息（message/context），并在读取后调用 `chaos_ffi_string_free(err)` 释放。

### 5.2 真实站点集成测试为什么要 feature gate？

真实站点可能受网络波动/风控/开播状态影响。默认 `cargo test` 必须稳定、离线可跑；
因此真实 URL 测试放在 `--features live-tests` 下，按需手动运行。
