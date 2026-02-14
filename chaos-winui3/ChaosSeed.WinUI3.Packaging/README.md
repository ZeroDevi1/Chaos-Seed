# ChaosSeed.WinUI3.Packaging (MSIX)

This folder contains a Windows Application Packaging Project (`.wapproj`) to build an MSIX for the WinUI3 app.

Notes:
- The `<Identity Publisher="...">` in `Package.appxmanifest` **must match** the subject of the signing certificate.
- CI expects `MSIX_PFX_BASE64` and `MSIX_PFX_PASSWORD` secrets, and will pass `/p:PackageVersion=<ver>` (e.g. `0.4.3.0`).
- CI generates an `AppInstaller` file that points to `releases/latest/download/...` so users can install once and keep updating.
