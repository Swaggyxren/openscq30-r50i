Instructions use Arch Linux package names (the primary target for this fork). Package names may differ on other distros.

If it's inconvenient to install the latest version of [just](https://github.com/casey/just), use the "without just" instructions.

## Building openscq30-cli on Linux

1. Install the latest version of rust

### Without just

2. Run `cargo build --package openscq30-cli --profile release-fast` (or `--release`, but that is very slow)
3. The compiled binary is at `target/release-fast/openscq30`

### With just

2. Run `just build-cli-fast` (or `just build-cli`)
3. The compiled binary is at `build-output/openscq30`

## Building openscq30-qt-gui on Linux

1. Install the latest version of rust
2. Install the Qt 6 and Kirigami build/runtime dependencies:

   ```sh
   sudo pacman -S qt6-base qt6-declarative kirigami
   ```

   `qt6-base` and `qt6-declarative` are needed to build the Rust ↔ Qt bridge; `kirigami`
   is the native KDE QML toolkit loaded at runtime.

### Without just

3. Run `cargo build --package openscq30-qt-gui --profile release-fast` (or `--release`)
4. The compiled binary is at `target/release-fast/openscq30-qt-gui`

### With just

3. Run `just build-gui-fast` (or `just build-gui`)
4. The compiled binary is at `build-output/openscq30-qt-gui`

## Runtime dependencies

- `kirigami` (the QML UI imports `org.kde.kirigami`)
- `breeze` (or another Qt Quick Controls 2 style) for the native KDE look
- A StatusNotifierItem host (KDE Plasma's system tray) for the tray menu
