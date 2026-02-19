# WinUI3 崩溃抓 Dump（WER LocalDumps）

当 `ChaosSeed.WinUI3.exe` 在弹窗/播放器等路径发生闪退（failfast / crash）时，Windows 事件查看器通常只给出 `Application Error (EventID=1000)`，不够定位问题。推荐启用 WER `LocalDumps` 生成 `*.dmp`。

## 1) 仅当前用户（推荐：不需要管理员权限）

在注册表中新建键：

- `HKEY_CURRENT_USER\Software\Microsoft\Windows\Windows Error Reporting\LocalDumps\ChaosSeed.WinUI3.exe`

并添加（DWORD / ExpandString）：

- `DumpType` (DWORD) = `2`（Full dump）
- `DumpCount` (DWORD) = `10`
- `DumpFolder` (ExpandString) = `%LOCALAPPDATA%\ChaosSeed\dumps`

## 2) 所有用户（需要管理员权限）

同样的值，写到：

- `HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\Windows Error Reporting\LocalDumps\ChaosSeed.WinUI3.exe`

## 3) 验证

1. 启用后，重现闪退
2. 在 `%LOCALAPPDATA%\ChaosSeed\dumps` 下找到最新的 `*.dmp`
3. 同时可查看日志：`%LOCALAPPDATA%\ChaosSeed\logs\winui3.log`

## 4) 注意

- Dump 可能很大（Full dump），建议只保留需要的版本/次数。
- 不要把包含敏感信息的日志/配置公开分享（例如 B 站 cookie/token）。

