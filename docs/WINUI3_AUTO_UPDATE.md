# WinUI3 自动更新（Zip + MSIX）

本仓库为 WinUI 3 应用提供两种更新通道：

## 1) Zip 自更新（便携版 / portable build）

- Release 产物：`chaos-winui3-windows-x86_64.zip`
- Hash 文件：`chaos-winui3-windows-x86_64.zip.sha256`

工作流程：
- WinUI3 会检查 GitHub `releases/latest` 是否存在更高版本。
- 点击 **Update now** 后，会下载 zip + sha256，先校验完整性，再启动 `ChaosSeed.Updater.exe`。
- Updater 会等待应用退出，替换应用目录中的文件，然后重启 WinUI3。

相关代码：
- `chaos-winui3/ChaosSeed.WinUI3/Services/UpdateService.cs`
- `chaos-winui3/ChaosSeed.Updater/Program.cs`

## 2) MSIX + AppInstaller（系统托管更新）

- Release 产物：
  - `ChaosSeed.WinUI3_x64.msix`
  - `ChaosSeed.WinUI3.appinstaller`
  - `ChaosSeed.WinUI3.cer`（若使用自签名证书，需要先安装该证书）

注意事项：
- `Package.appxmanifest` 里的 MSIX Identity Publisher 必须与签名证书一致。
- MSIX 安装包 **不会** 使用 Zip 自更新；更新由 Windows / AppInstaller 托管。
