# Chaos Seed Wiki

本目录包含 Chaos Seed 项目的 Wiki 文档。

## 文档列表

- [Home.md](Home.md) - 项目主页与快速开始
- [BUILD_WINUI3.md](BUILD_WINUI3.md) - WinUI3 构建指南
- [FFI_API.md](FFI_API.md) - FFI API 参考
- [FFI_BUILD.md](FFI_BUILD.md) - FFI 构建说明
- [FFI_CSharp.md](FFI_CSharp.md) - C# FFI 使用指南
- [Daemon_API.md](Daemon_API.md) - Daemon JSON-RPC API
- [Daemon_CSharp.md](Daemon_CSharp.md) - C# Daemon 客户端
- [DEVLOG.md](DEVLOG.md) - 开发日志
- [TODO.md](TODO.md) - 待办事项
- [TODO_NEXT.md](TODO_NEXT.md) - 后续计划

## 分支结构

本项目采用多分支架构：

```
main-core (核心库)
├── winui3 (WinUI3 桌面端)
├── tauri (Tauri 桌面端)
├── slint (Slint Native UI)
├── flutter (Flutter 跨平台)
├── android (Android 原生)
└── cli (命令行工具)
```

每个分支包含完整的代码和独立的构建配置。

## 同步说明

Wiki 文档与代码仓库分离管理。如需更新 Wiki：

1. 修改 `docs/wiki/` 目录下的文件
2. 提交并推送
3. 如需同步到 GitHub Wiki，请参考 `docs/WIKI_SYNC.md`
