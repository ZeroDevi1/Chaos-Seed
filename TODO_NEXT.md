# TODO_NEXT（近期 1~5 天交付）

> 说明：这里仅放“最近几天内必须交付”的 bugfix/小功能/风险修复。条目数量保持少（建议 3~6 个），但每条要写清：现象、实现方向（概括）、验收标准、回归清单。

## Done（已完成）

> 日期统一按你确认：2026-02-07。commit hash 为“最贴近该功能完成”的提交（允许多个条目引用同一个提交）。

- P0-1 启动白屏 / 首屏延迟 2~3s：Done @ 2026-02-07（commit: `6e4d711`）
- P0-2 搜索按钮点击后闪退：Done @ 2026-02-07（commit: `6e4d711`）
- P1-4 白天模式按钮不可见 + 输入框/说明不清晰：Done @ 2026-02-07（commit: `6e4d711`）
- P2-5 Windows 图标（Explorer/任务栏/Alt-Tab）：Done @ 2026-02-07（commit: `6e4d711`）
- P2-6 About 链接 + 侧边栏 Win11 观感增强：Done @ 2026-02-07（commit: `6e4d711`）

## Next（近期要交付）

### P0：字幕下载页 - 搜索有结果但列表不显示 + 输入框鼠标点击不聚焦

**现象**
- 日志显示 `task=search ok items>0` 且 `ui=results_update count>0`，但 UI 列表不显示。
- 鼠标点击输入框无反应（已修复）：必须 Tab 切换才能输入/定位光标。

**实现方向（概括）**
- 优先修复 `ui/app.slint`：对每个页面 View 实例显式设置 `width/height = parent.width/parent.height`，修复布局与命中测试区域异常。
- 若仍不显示：Rust 侧改为稳定 `ModelRc`（只 set 一次，后续更新 `VecModel` 内容，不替换 model 指针）。
- （可选）UI 增加 debug 字段显示 `results.length`，快速判断数据是否传入。
- （诊断增强）搜索成功后把 items（最多 50 条）dump 到 `logs/app.log`，用于确认是否真正拿到数据。

**验收标准**
- 鼠标点击输入框立刻聚焦并可输入，光标可随鼠标定位。
- 搜索后列表出现 N 行（例如 20 行），每行“下载”按钮可点击。

**回归清单**
- `cargo test`
- Windows 原生：`cargo build --release`
- WSL -> windows-gnu：`cargo build --release --target x86_64-pc-windows-gnu`
- WSL -> windows-msvc（xwin）：`cargo xwin build --release --target x86_64-pc-windows-msvc`

---

### P1：新增页面占位（Settings / 直播源 / 弹幕）+ 侧边栏导航调整

**交付目标**
- 在侧边栏新增入口：直播源、弹幕、设置（占位即可），About 保持在底部。
- 页面只需要 icon+名称+空页面/占位文案即可。

**验收标准**
- 能正常切换到 3 个新页面，About 仍在底部且链接可用。

---

### P2：弹幕 - 抽象层与两种 UI 形态设计冻结（本次不实现）

**目标（本次只写方案，不写实现代码）**
- 把“平台解析抽象 + 连接弹幕流 + 两种展示形态 + 置顶/透明策略”写到可直接开工实现的程度。

**抽象建议（后续实现用）**
- `PlatformResolver`：按平台匹配 URL 并解析得到统一的 `ResolvedRoom`
- `ResolvedRoom`：`platform/room_id/connect_info`
- `DanmakuClient`：连接弹幕流，输出统一 `DanmakuMsg(user/text/ts/...)`

**两种展示形态**
- 竖直 Chat：背景不透明、从下往上滚动、显示用户名+弹幕、支持置顶。
- 水平 Overlay：背景透明、从右往左滚动、位于屏幕上半部分、支持置顶（可在 A 直播间叠 B 弹幕）。

**窗口能力**
- Slint `Window` 支持 `always-on-top`
- 透明背景优先 `background: #00000000`，不稳定则 fallback 半透明深色背景

**验收标准**
- 文档/接口定义完善到“直接开工实现”的程度（不涉及逐行编码细节）。

---

### P2：Setting - 日志输出开关（下次做）

**需求**
- Setting 页面提供开关：是否输出 `logs/app.log`（建议 Debug 默认开启，Release 默认关闭）。
