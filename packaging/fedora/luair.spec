Name:           luair
Version:        2.0.2
Release:        1%{?dist}
Summary:        Independent desktop crosshair overlay powered by Rust and Tauri v2

License:        MIT
URL:            https://github.com/confusionszn/luair
Source0:        %{url}/archive/refs/tags/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  nodejs
BuildRequires:  pkgconfig(gtk+-3.0)
BuildRequires:  pkgconfig(webkit2gtk-4.1)
BuildRequires:  pkgconfig(ayatana-appindicator3-0.1)
BuildRequires:  pkgconfig(x11)
BuildRequires:  pkgconfig(xtst)
BuildRequires:  pkgconfig(openssl)

Requires:       gtk3
Requires:       webkit2gtk4.1
Requires:       libappindicator-gtk3
Requires:       libX11
Requires:       libXtst
Requires:       openssl-libs

%description
Luair is an independent, high-performance desktop crosshair overlay built with
Rust and Tauri v2. It renders customizable click-through overlays over X11 and
Wayland desktops without hooking or injecting into game processes, and includes
dynamic glassmorphism themes, an encrypted local account vault, and an embedded
Candle GGUF AI assistant.

%prep
%autosetup -n %{name}-%{version}

%build
npm install -g pnpm || true
pnpm install --frozen-lockfile || pnpm install
pnpm run build
cargo build --manifest-path src-tauri/Cargo.toml --release

%install
install -Dm0755 src-tauri/target/release/luair %{buildroot}%{_bindir}/luair
install -Dm0644 packaging/linux/com.luair.overlay.desktop \
  %{buildroot}%{_datadir}/applications/com.luair.overlay.desktop
install -Dm0644 src-tauri/icons/128x128.png \
  %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/com.luair.overlay.png
install -Dm0644 public/luair-icon.svg \
  %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/com.luair.overlay.svg

%files
%{_bindir}/luair
%{_datadir}/applications/com.luair.overlay.desktop
%{_datadir}/icons/hicolor/128x128/apps/com.luair.overlay.png
%{_datadir}/icons/hicolor/scalable/apps/com.luair.overlay.svg

%changelog
* Mon Nov 23 2026 Luair Contributors <maintainers@luair.app> - 2.0.2-1
- Fix Windows 10 overlay transparency, reliable F10 global menu toggle, standard DIB/PNG icon.ico, and frameless launcher edge artifacts

* Sun Nov 22 2026 Luair Contributors <maintainers@luair.app> - 2.0.1-1
- Align WebView2 additionalBrowserArgs across overlay and menu windows

* Sun Nov 22 2026 Luair Contributors <maintainers@luair.app> - 2.0.0-1
- Initial Tauri v2 + Rust cross-platform release with in-app GitHub Releases updater
