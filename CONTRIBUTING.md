# Contributing to Luair

Thank you for helping improve **Luair**! Before submitting an issue or pull
request, please review our engineering principles and local setup instructions.

## Core Design Principles

1. **Strictly Independent Desktop Overlay**: Luair renders on an isolated
   transparent desktop window above games. Do **not** submit code that reads
   game memory, injects libraries/DLLs, enumerates game processes, or modifies
   game files.
2. **Open & Self-Contained**: No telemetry, no external account servers, no key
   walls or artificial feature locks. All profiles, vaults, and Candle GGUF
   AI inference run locally.
3. **Cross-Platform Fidelity**: Changes to window management, capture cloaking,
   or hotkeys must remain compatible across **Windows (MSVC x86_64)**, **Linux
   (X11 & Wayland)**, and **macOS Universal (Intel + Apple Silicon)** via
   appropriate `#[cfg(target_os = ...)]` guards.

## Local Development Prerequisites

### All Platforms
* **Rust toolchain** (`stable` via `rustup`)
* **Node.js 22+** and **pnpm 10+**

### Windows (`x86_64-pc-windows-msvc`)
* Visual Studio Build Tools 2022 (MSVC C++ toolset & Windows SDK)
* Microsoft Edge WebView2 Runtime (built into Windows 10/11)

### Linux (`x86_64-unknown-linux-gnu`)
Install GTK3, WebKitGTK 4.1, tray indicator, X11/XTest, and OpenSSL dev headers:
* **Debian / Ubuntu**:
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev patchelf libx11-dev libxtst-dev \
    libxi-dev libssl-dev pkg-config rpm
  ```
* **Fedora**:
  ```bash
  sudo dnf install -y webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
    libX11-devel libXtst-devel openssl-devel rpm-build pkgconf
  ```
* **Arch Linux**:
  ```bash
  sudo pacman -S --needed webkit2gtk-4.1 gtk3 libayatana-appindicator libx11 \
    libxtst openssl pkgconf
  ```

### macOS (`universal-apple-darwin`)
* Xcode Command Line Tools (`xcode-select --install`)
* Both targets installed for universal builds:
  ```bash
  rustup target add x86_64-apple-darwin aarch64-apple-darwin
  ```

## Development Workflow

```bash
# Install frontend dependencies
pnpm install

# Launch desktop app in hot-reload mode (Vite + Rust)
pnpm tauri dev

# Run frontend TypeScript check & Vite bundle
pnpm run build

# Run Rust unit tests and clippy / compiler checks
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

## Pull Request Checklist

* `pnpm run build` (`tsc --noEmit && vite build`) passes with zero errors.
* `cargo test --manifest-path src-tauri/Cargo.toml` passes all unit tests with
  zero compiler warnings.
* New Rust commands include unit tests covering pure logic and edge cases.
* Keep PRs focused and concise, and reference the related issue when opening a PR.
