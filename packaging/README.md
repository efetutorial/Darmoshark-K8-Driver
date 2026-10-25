# Packaging

On Windows, `tauri.windows.conf.json` selects an NSIS installer and the Windows
ICO asset. Build from `src-tauri/` with `cargo tauri build --bundles nsis -- --locked`.
The installer uses the current user's installation directory and bootstraps
WebView2 when needed. Windows uses the stock HID driver; the Linux udev rule is
not installed. CI uploads the installer and standalone `.exe` as an artifact;
tagged builds add the installer to the GitHub release.

Tauri reads `src-tauri/tauri.conf.json` and produces Debian, RPM and AppImage
bundles. Build them from `src-tauri/` with:

```console
cargo tauri build --bundles deb,rpm,appimage
```

The Debian and RPM bundles include the udev rule from this directory.

`arch/PKGBUILD.in` is a release template. The GitHub workflow replaces its
version, repository and checksum placeholders and attaches the resulting
`PKGBUILD` to the release.

The SVG icon and desktop entry are shared by all package formats. Keep their
application ID and executable name aligned with `src-tauri/tauri.conf.json` and
`src-tauri/Cargo.toml`.
