# OpenSCQ30 — R50i NC

A personal fork of [OpenSCQ30](https://github.com/Oppzippy/OpenSCQ30) that controls settings for a **single device**: the Soundcore **R50i NC** (internal model id `SoundcoreA3959`). Every other Soundcore model has been removed to keep the codebase and the UI simple.

## Features

- **Native Qt/QML desktop app** built on [Kirigami](https://develop.kde.org/frameworks/kirigami/) for KDE Plasma (Wayland) — launches straight into the R50i NC settings and auto-connects on startup.
- **KDE system tray** (`ksni` StatusNotifierItem): ANC mode (Normal / Transparency / Noise Cancelling), live battery level, "Open Settings", and "Quit".
- **CLI** (`openscq30`) for scripting and quick control.
- **Android app** built on the same Rust core (kept for completeness; the desktop app is the primary target).

## Supported device

| Model | Name              |
| ----- | ----------------- |
| A3959 | Soundcore R50i NC |

## Building

Requirements: Rust ≥ 1.85 (edition 2024), Qt 6, and Kirigami. On Arch Linux:

```sh
sudo pacman -S qt6-base qt6-declarative kirigami
```

Build the GUI and CLI:

```sh
cargo build -p openscq30-qt-gui -p openscq30-cli
# -> target/debug/openscq30-qt-gui  and  target/debug/openscq30
```

Run tests:

```sh
cargo test --workspace
```

## Usage

```sh
# GUI (auto-connects to the paired R50i NC; shows the pairing screen if none is paired)
cargo run -p openscq30-qt-gui

# CLI: pair a device (separate from OS Bluetooth pairing)
openscq30 paired-devices add --mac-address <MAC> --model SoundcoreA3959

# CLI: list settings and change the ANC mode
openscq30 device --mac-address <MAC> list-settings
openscq30 device --mac-address <MAC> setting --set ambientSoundMode=NoiseCanceling
```

Add `--demo` to `paired-devices add` to use a virtual device without hardware connected.

## Notes

- This is a **personal fork** for one device; it is not intended to be merged back upstream.
- Closing the GUI window hides it to the system tray rather than quitting — use the tray's "Quit" item (or the app's drawer) to exit.

## License

[GPL-3.0-or-later](LICENSE.txt), inherited from upstream.
