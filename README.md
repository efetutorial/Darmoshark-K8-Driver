This driver was written entirely by ai, and it works better than the official one

# Darmoshark K8 Studio


Linux and Windows control application for the Darmoshark K8 keyboard, built with Rust and
[GPUI](https://gpui.rs). The interface is drawn natively on the GPU; there is no web view.

![Linux](https://img.shields.io/badge/platform-Linux-blue)
![Windows](https://img.shields.io/badge/platform-Windows-blue)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20GPUI-orange)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- Onboard profile management
- RGB effects, brightness, speed, direction and supported color controls
- Per-key color and key remapping
- Hardware macro editing; on Linux, recording reads the K8 directly and works with any keyboard layout
- 125, 250, 500 and 1000 Hz report-rate settings
- Debounce, sleep, battery and firmware information
- 128×128 LCD image and GIF upload
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
| Other glibc-based distributions | `_linux_x86_64.tar.gz` |
| Arch Linux and derivatives | `PKGBUILD` |
| Windows 10 / 11 (x64) | `-windows-x86_64.exe` |

### Windows

Download the `.exe` and run it; no installation or runtime is needed. No udev
rule, Zadig, or custom USB driver is needed. Connect the K8 by USB and switch it
to wired mode. Close the manufacturer's driver before opening Studio to avoid
competing writes.

Windows builds use the native HID API for keyboard control, GDI for primary
screen-color sync, and WASAPI loopback for the default playback device's audio
spectrum. Windows hardware behavior still needs validation on a Windows PC.

### Linux

The Debian and RPM packages install the keyboard udev rule. Tarball users must
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

Install stable Rust. Linux needs a working Vulkan driver at runtime.

### Windows

Install the MSVC toolchain (Visual Studio Build Tools with **Desktop
development with C++** and a Windows SDK), then run in PowerShell:

```powershell
git clone https://github.com/efetutorial/Darmoshark-K8-Driver.git
cd Darmoshark-K8-Driver
cargo test --locked
cargo build --release --locked
```

The executable is written to `target/release/darmoshark-k8-studio.exe`.

### Linux

Debian and Ubuntu:

```console
sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libvulkan-dev libx11-dev libx11-xcb-dev libxcb1-dev \
  libfontconfig1-dev libfreetype-dev
```

Fedora:

```console
sudo dnf install cargo rust gcc pkgconf libxkbcommon-devel libxkbcommon-x11-devel \
  wayland-devel vulkan-loader-devel libX11-devel libxcb-devel fontconfig-devel freetype-devel
```

Arch Linux:

```console
sudo pacman -S --needed base-devel cargo rust libxkbcommon libxkbcommon-x11 wayland \
  vulkan-icd-loader libx11 libxcb fontconfig freetype2
```

Then build:

```console
git clone https://github.com/efetutorial/Darmoshark-K8-Driver.git
cd Darmoshark-K8-Driver
cargo build --release --locked
```

The executable is written to `target/release/`. See
[packaging/README.md](packaging/README.md) to build `.deb` and `.rpm` packages.

On NixOS, `nix build` produces a ready-to-run package, and `nix develop` provides
a shell with the build and runtime libraries.

## Optional integrations

- On Linux, `grim` is required for screen capture on Wayland.
- On Linux, PipeWire's `pw-cat` is required for audio-spectrum lighting.
- On Windows, screen and playback capture use built-in APIs.

In **Display**, select a photo or GIF and upload it. Photos replace only the
selected slot. GIFs retain their animation; the slot selector applies to still
images. Use **Fn + Backspace** on the keyboard to switch stored pictures.
The app no longer converts GIFs into still images or fills all slots implicitly.

## Development

```console
cargo fmt -- --check
cargo test --locked
nix flake check
```

`--page <lighting|display|keys|macros|settings>` opens the app on a specific page.

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and
[docs/hardware-notes.md](docs/hardware-notes.md) for protocol notes.

## License

[MIT](LICENSE)
