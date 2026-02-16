# MPV / libmpv (Android) 合规说明（chaos-android）

本仓库的 `chaos-android` 模块支持两套播放器引擎：
- EXO：AndroidX Media3 ExoPlayer
- MPV：通过 `dev.jdtech.mpv:libmpv:0.5.1` 集成 `libmpv`（AAR 内包含 mpv/ffmpeg 等原生库）

## 重要提示（非法律意见）

`libmpv`/`mpv`/`ffmpeg` 及其依赖可能涉及 GPL/LGPL 等许可证义务（例如：发布时提供许可证文本、提供对应源码/修改内容、动态/静态链接差异等）。

本文件用于记录工程侧的合规交付物与流程，不构成法律建议。对外发布前请自行核对最终打包产物所包含的库与其许可证要求。

## 本仓库的合规交付物（工程约束）

1) App 内第三方声明：
- `chaos-android/app/src/main/assets/third_party_notices.txt`
- 设置页提供入口：`开源许可与第三方声明`

2) 依赖来源固定：
- `chaos-android/app/build.gradle.kts` 中使用 `dev.jdtech.mpv:libmpv:0.5.1`

3) 发布前检查清单（建议）
- 确认最终 APK/AAB 中包含的 native so 列表（mpv/ffmpeg 等）
- 收集并附带对应许可证文本（GPL/LGPL/BSD/MIT 等）
- 若你对 mpv/ffmpeg 或其依赖进行了修改：提供修改后的源码与构建方式
- 若许可证要求提供“可获取源码”的方式：在仓库 README 或发布页提供清晰链接/说明

## 变更记录

- 2026-02-16：引入 MPV 双引擎（EXO/MPV）并增加 third_party_notices 占位文件。

