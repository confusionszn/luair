# Luair Cross-Platform Build & Updater Guide

Luair v2 targets **Windows (MSVC x86_64)**, **Linux (Debian, Arch, Fedora, AppImage, Flatpak)**, and **macOS Universal (Intel + Apple Silicon)** from a single Rust + Tauri v2 codebase.

## 1. Windows (`x86_64-pc-windows-msvc`)
- **Overlay & Capture Cloak**:
  - Uses Win32 `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED`, `HWND_TOPMOST`, and DWM `DwmExtendFrameIntoClientArea` (`-1` margins) + `DWMWA_TRANSITIONS_FORCEDISABLED` for flicker-free DirectX borderless-windowed and GDI compositing.
  - Uses `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` to hide the crosshair (and optionally the menu) from OBS, Discord, and screenshots.
- **Build Command**:
  ```powershell
  pnpm.cmd run tauri:build:win
  ```

## 2. Linux (`x86_64-unknown-linux-gnu`, X11 & Wayland)
- **System Libraries**:
  - Debian / Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libx11-dev libxtst-dev libssl-dev`
  - Fedora / RHEL: `webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel libX11-devel libXtst-devel openssl-devel rpm-build`
  - Arch Linux: `webkit2gtk-4.1 gtk3 libayatana-appindicator libx11 libxtst openssl`
- **Packaging**:
  - Native DEB, RPM, & AppImage bundles: `pnpm.cmd run tauri:build:linux`
  - Arch Linux `PKGBUILD`: [`packaging/arch/PKGBUILD`](./arch/PKGBUILD) (`makepkg -si`)
  - Fedora RPM spec: [`packaging/fedora/luair.spec`](./fedora/luair.spec)
  - Flatpak manifest (Wayland + fallback X11 sockets): [`packaging/flatpak/com.luair.overlay.yml`](./flatpak/com.luair.overlay.yml)
- **Display Server Notes**:
  - Supports transparent click-through overlays and `set_visible_on_all_workspaces(true)` across X11 and Wayland desktops.
  - On Wayland compositors where XWayland/uinput limits global hotkeys, XWayland or input-group permissions allow full non-blocking slot hotkeys.

## 3. macOS Universal (`universal-apple-darwin`)
- **Targets**: Combines `x86_64-apple-darwin` (Intel) and `aarch64-apple-darwin` (Apple Silicon) with minimum macOS `11.0`, `macOSPrivateApi: true` transparency, [`src-tauri/entitlements.plist`](../src-tauri/entitlements.plist), and [`src-tauri/Info.plist`](../src-tauri/Info.plist).
- **Capture Cloak**: Uses AppKit `NSWindowSharingNone` via `set_content_protected(true)`.
- **Build Command**:
  ```bash
  rustup target add x86_64-apple-darwin aarch64-apple-darwin
  pnpm run tauri:build:macos
  ```

## 4. In-App Auto-Updater Signing Setup
1. Generate a production Minisign keypair (keep the private key secret in CI):
   ```bash
   pnpm tauri signer generate -w ~/.tauri/luair.key
   ```
2. Replace `plugins.updater.pubkey` in [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) with the generated `.pub` base64 string.
3. Add `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to GitHub Actions Secrets so [`release.yml`](../.github/workflows/release.yml) publishes signed bundles and `latest.json` automatically to GitHub Releases.
