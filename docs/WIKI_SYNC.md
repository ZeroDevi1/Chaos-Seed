# Wiki 同步（GitHub Actions）

本仓库把 Markdown 作为唯一真相，通过 GitHub Actions **单向镜像覆盖**到 GitHub Wiki（`<repo>.wiki.git`）。

## 行为概述

- 触发：`main` 分支的文档变更（或手动 `workflow_dispatch`）
- 导出：运行 `scripts/wiki_export.sh` 生成 `.wiki-export/`
- 同步：将 `.wiki-export/` **镜像**到 Wiki 仓库（包含删除 Wiki 中多余页面）
- Home：`README.md` → `Home.md`（即 Wiki 的 `Home`）

## 页面映射（allowlist）

> 只同步下面这些文件，避免把 `refs/**` 等内容误推送到 Wiki。

| 仓库源文件 | Wiki 目标文件 | Wiki 页面名 |
|---|---|---|
| `README.md` | `Home.md` | `Home` |
| `docs/BUILD_WINUI3.md` | `BUILD_WINUI3.md` | `BUILD_WINUI3` |
| `chaos-ffi/docs/API.md` | `FFI_API.md` | `FFI_API` |
| `chaos-ffi/docs/CSharp.md` | `FFI_CSharp.md` | `FFI_CSharp` |
| `chaos-ffi/docs/BUILD.md` | `FFI_BUILD.md` | `FFI_BUILD` |
| `chaos-daemon/docs/API.md` | `Daemon_API.md` | `Daemon_API` |
| `chaos-daemon/docs/CSharp.md` | `Daemon_CSharp.md` | `Daemon_CSharp` |
| `TODO.md` | `TODO.md` | `TODO` |
| `TODO_NEXT.md` | `TODO_NEXT.md` | `TODO_NEXT` |
| `DEVLOG.md` | `DEVLOG.md` | `DEVLOG` |

Sidebar：由导出脚本生成 `_Sidebar.md`，使用 `[[Page]]` 导航。

## 本地运行

在仓库根目录执行：

```bash
bash scripts/wiki_export.sh .wiki-export
ls -la .wiki-export
```

## 鉴权与权限

默认 workflow 先使用 `GITHUB_TOKEN` 访问 `*.wiki.git`：
- 如果你的仓库配置允许，Actions 可以直接 push wiki。
- 如果遇到权限问题（clone/push 失败），改用 PAT 即可（无需调整同步逻辑）。

### 备用：Fine-grained PAT

1. 创建 Fine-grained PAT（仅此仓库，`Contents: Read and write`）
2. 添加到仓库 `Secrets`：`WIKI_TOKEN`
3. workflow 会自动优先使用 `WIKI_TOKEN`（否则回退到 `GITHUB_TOKEN`）

## 风险与回退

- 风险：镜像模式会删除 Wiki 中“非 allowlist 导出”的页面；不要手工维护 Wiki 内容。
- 回退：禁用 `.github/workflows/wiki.yml`，或移除 `rsync --delete`（会导致旧页面残留）。
