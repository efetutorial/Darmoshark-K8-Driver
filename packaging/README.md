# Packaging

The application is a single native executable built with `cargo build --release`.
There is no web runtime: the UI is drawn with GPUI.

## Linux

Debian and RPM packages are built with
[cargo-deb](https://github.com/kornelski/cargo-deb) and
[cargo-generate-rpm](https://github.com/cat-in-136/cargo-generate-rpm), using the
metadata in `Cargo.toml`:

```console
cargo build --release --locked
cargo deb --no-build
cargo generate-rpm
```

Both packages install the udev rule from this directory. GPUI loads Vulkan,
Wayland and X11 libraries at runtime, so the package dependencies list them
explicitly. A working Vulkan driver is required.

`arch/PKGBUILD.in` is a release template. The release workflow replaces its
version, repository and checksum placeholders and attaches the resulting
`PKGBUILD` to the release. The Nix flake builds the same binary and adds the
runtime libraries to its rpath.

## Windows

`cargo build --release --locked` produces
`target/release/darmoshark-k8-studio.exe` with the icon from
`assets/icons/icon.ico` embedded by `build.rs`. Windows uses the stock HID
driver; the Linux udev rule is not needed. No installer is produced.

The SVG icon and desktop entry are shared by the Linux package formats. Keep
their application ID (`darmoshark-k8`) aligned with the `app_id` in `src/ui/mod.rs`.
