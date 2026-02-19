# B站视频下载（WinUI3）

## 登录类型说明

- **Web 扫码登录**：会得到 `Cookie/refresh_token`（用于 Web API）。支持“刷新 Cookie”。
- **TV 扫码登录**：会得到 `access_token`（用于 TV API）。**不支持**“刷新 Cookie”（因为它不是 Web Cookie）。

当前版本在 **仅 TV 登录** 的情况下，开始下载会自动使用 **`api=tv`**，避免 Web API 返回“未登录”。

## 常见问题

### 1) “刷新 Cookie”提示未登录

如果你只做了 **TV 登录**，这是预期行为：TV 登录没有 Web Cookie 可刷新。

解决：
- 需要 Web Cookie 时：点“Web 扫码登录”
- 仅用于下载：直接点“开始下载”（会自动走 `api=tv`）

### 2) 点击下载后一直 0 B / ? 没进度

建议按顺序排查：

1. 看任务条目是否有 `Error` 文本（失败原因会写在这里）。
2. 打开 WinUI3 日志：`%LOCALAPPDATA%\\ChaosSeed\\logs\\winui3.log`
   - 搜索 `bili.task.add`，可看到任务启动时使用的 `api/web/tv` 与输出目录等信息。
3. 若仍无进度，优先检查网络/代理/防火墙（以及能否直接访问 B 站与视频 CDN 域名）。

