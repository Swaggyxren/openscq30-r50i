## Building openscq30-cli on Windows

1. Install rust
2. Run `cargo build --package openscq30-cli --profile release-fast` (or `--release`, but that is very slow)
3. The compiled binary is at `.\target\release-fast\openscq30.exe`

## Desktop GUI

The desktop GUI (`openscq30-qt-gui`) is a native Qt/Kirigami app targeting **Linux** (KDE Plasma).
It is not built for Windows in this fork — see `docs/build-linux.md`.
