This driver was written entirely by ai, and it works better than the official one

# Darmoshark K8 Studio


Linux control application for the Darmoshark K8 keyboard, built with Rust and
Tauri.

![Linux](https://img.shields.io/badge/platform-Linux-blue)
![Rust](https://img.shields.io/badge/backend-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- Onboard profile management
- RGB effects, brightness, speed, direction and supported color controls
- Per-key color and key remapping
- Hardware macro editing
- 125, 250, 500 and 1000 Hz report-rate settings
- Debounce, sleep, battery and firmware information
- 128×128 LCD image and GIF upload
- LCD information cards, screenshots and clock synchronization
- Screen-color and audio-spectrum lighting modes

## Supported hardware

| Device | USB ID | Vendor usage |
| --- | --- | --- |
| Darmoshark K8 | `3151:4015` | `ffff:0002` |

Other models have not been tested.

## Installation

Download a package from the
[latest GitHub release](https://github.com/efetutorial/Darmoshark-K8-Driver/releases/latest):

| Distribution | File |
| --- | --- |
| Debian, Ubuntu, Linux Mint, Pop!_OS | `.deb` |
| Fedora, openSUSE, RHEL derivatives | `.rpm` |
| Other glibc-based distributions | `.AppImage` |
| Arch Linux and derivatives | `PKGBUILD` |

The Debian and RPM packages install the keyboard udev rule. AppImage users must
install it separately:

```console
curl -LO https://github.com/efetutorial/Darmoshark-K8-Driver/releases/latest/download/99-darmoshark-k8.rules
sudo install -Dm644 99-darmoshark-k8.rules /etc/udev/rules.d/99-darmoshark-k8.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Reconnect the keyboard after installing the rule.

### Nix and NixOS

Run or install the application directly from the repository:

```console
nix run github:efetutorial/Darmoshark-K8-Driver
nix profile install github:efetutorial/Darmoshark-K8-Driver
```

The included NixOS module installs both the application and its udev rule:

```nix
{
  inputs.darmoshark-k8.url = "github:efetutorial/Darmoshark-K8-Driver";

  outputs = { nixpkgs, darmoshark-k8, ... }: {
    nixosConfigurations.my-machine = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        darmoshark-k8.nixosModules.default
        ./configuration.nix
      ];
    };
  };
}
```

## Building from source

Install stable Rust and the Linux libraries required by Tauri 2.

Debian and Ubuntu:

```console
sudo apt update
sudo apt install build-essential curl file libappindicator3-dev libgtk-3-dev \
  librsvg2-dev libsoup-3.0-dev libwebkit2gtk-4.1-dev patchelf
```

Fedora:

```console
sudo dnf install cargo rust gcc-c++ gtk3-devel libappindicator-gtk3-devel \
  librsvg2-devel libsoup3-devel openssl-devel webkit2gtk4.1-devel
```

Arch Linux:

```console
sudo pacman -S --needed base-devel cargo gtk3 libappindicator-gtk3 libsoup3 \
  librsvg rust webkit2gtk-4.1
```

Install Tauri CLI and build the application:

```console
cargo install tauri-cli --version '^2' --locked
git clone https://github.com/efetutorial/Darmoshark-K8-Driver.git
cd Darmoshark-K8-Driver/src-tauri
cargo tauri build --no-bundle
```

The executable is written to `src-tauri/target/release/`. Build a distribution
package with one of the following commands:

```console
cargo tauri build --bundles deb
cargo tauri build --bundles rpm
cargo tauri build --bundles appimage
```

Packages are written below `src-tauri/target/release/bundle/`.

On NixOS:

```console
git clone https://github.com/efetutorial/Darmoshark-K8-Driver.git
cd Darmoshark-K8-Driver
nix develop
cd src-tauri
cargo tauri build --no-bundle
```

## Optional integrations

- `grim` is required for screen capture on Wayland.
- PipeWire's `pw-cat` is required for audio-spectrum lighting.

LCD screen capture uploads a single frame. Continuous desktop mirroring is not
available because the keyboard protocol stores complete frames through HID
packets instead of exposing a live framebuffer.

## Development

```console
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm ci
npx playwright install chromium
npm test
nix flake check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and
[docs/hardware-notes.md](docs/hardware-notes.md) for protocol notes.

## License

[MIT](LICENSE)
