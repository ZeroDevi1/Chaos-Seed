# WinUI3 Auto Update (Zip + MSIX)

This repo supports two update channels for the WinUI 3 app:

## 1) Zip self-updater (portable build)

- Release asset: `chaos-winui3-windows-x86_64.zip`
- Hash file: `chaos-winui3-windows-x86_64.zip.sha256`

How it works:
- WinUI3 checks GitHub `releases/latest` for a newer version.
- When you click **Update now**, it downloads the zip + sha256, verifies integrity, then launches `ChaosSeed.Updater.exe`.
- The updater waits for the app to exit, replaces files in the app folder, and restarts WinUI3.

Files involved:
- `chaos-winui3/ChaosSeed.WinUI3/Services/UpdateService.cs`
- `chaos-winui3/ChaosSeed.Updater/Program.cs`

## 2) MSIX + AppInstaller (system-managed updates)

- Release assets:
  - `ChaosSeed.WinUI3_x64.msix`
  - `ChaosSeed.WinUI3.appinstaller`
  - `ChaosSeed.WinUI3.cer` (install this cert first if using a self-signed cert)

Notes:
- The MSIX identity publisher in `Package.appxmanifest` must match the signing certificate.
- Packaged installs do **not** use the zip self-updater; Windows manages updates via AppInstaller.

