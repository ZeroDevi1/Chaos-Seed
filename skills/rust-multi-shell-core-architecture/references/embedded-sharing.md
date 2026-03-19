# 面向 STM32 / ESP32 的抽核规则

## 默认立场

不要把整个应用核心 crate 直接搬到 MCU。
把目标设成“从现有核心 crate 中继续抽出 alloc-friendly、尽量 no_std-friendly 的共享 crate”。

## 更适合抽出的逻辑

- 纯采样或评分算法
  - 例如类似 `tts::sampling` 的纯数值计算
- 纯文本规范化、规则拼装
  - 例如类似 `tts::text` 的 prompt 组装思路
- 纯音频后处理
  - 例如类似 `tts::post_process` 的裁剪策略
- 纯数据筛选与排序规则
  - 例如类似 `subtitle::core::apply_filters` 的筛选函数
- 纯模型和值对象
  - 例如直播清晰度、结果项、状态枚举一类的数据模型

## 需要谨慎抽出的逻辑

- 轻量 VAD、parser、codec glue
  - 算法本身可能较轻，但要检查是否偷带 `std`、文件、日志或外部后端假设
- 某些 parser / model
  - 如果当前实现夹带网络请求、URL 解析、时间源或 serde-only 约束，要先拆再共享

## Chaos-Seed 示例

- `tts::sampling`、`tts::text`、`tts::post_process` 更接近可继续抽小 crate 的方向
- `subtitle::core::apply_filters` 属于可迁移的数据规则
- `tts::vad::EnergyVad` 可以作为谨慎抽取对象
- `python_runner`、Now Playing、daemon 传输层明显应留在桌面适配层

## 明显不适合直接给 MCU 的逻辑

- `reqwest` / `tokio` 驱动的网络客户端
- Windows API、Now Playing、SMTC 一类平台能力
- Python runner、子进程推理、PyO3 路线
- daemon、NamedPipe、JSON-RPC/LSP framing
- 依赖桌面文件系统、下载目录、Cookie 持久化的能力
- 需要大模型、重型解码器或桌面级依赖的流程

## 抽新 crate 的规则

- 先从现有模块中切出“纯算法 + 纯模型”最小闭包
- 让共享 crate 默认只依赖 `core`、`alloc` 或少量轻依赖
- 把 `serde`、`std`、平台绑定做成 feature gate 或外围适配层
- 把时间、随机、网络、日志、存储、音频输入输出通过 trait 注入
- 让桌面侧核心 crate 与 MCU HAL 侧同时依赖这个新 crate，而不是互相依赖

## 设计检查清单

- 这段逻辑是否离开文件系统和网络后仍成立
- 这段逻辑是否能只依赖输入数据与配置完成计算
- 这段逻辑是否必须感知线程、进程、窗口或 ABI
- 这段逻辑是否可以把外部副作用延后到适配层
- 这段逻辑是否值得为共享而新建更小的 crate

## 一句话决策法

如果它回答的是“怎么算”，尽量往共享 crate 抽。
如果它回答的是“怎么连平台、怎么跑后台、怎么跨边界”，就留在桌面适配层。
