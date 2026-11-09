# myDupFinder UI (Rusty)

A native, read-only desktop viewer for the `dupReport.csv` files produced by
[myDupFinder](https://github.com/rogerbriggen/myDupFinder). It rebuilds the
folder tree from the report, shows the files of the selected folder in a
virtualised table with filters, and detects folders that are completely
identical across two scans.

Built in pure Rust with [GPUI](https://www.gpui.rs/) and
[gpui-component](https://github.com/longbridge/gpui-kit). No webview, no JavaScript.

Status: early development. The workspace skeleton is in place; see
[`docs/REBUILD_RUST_GPUI.md`](docs/REBUILD_RUST_GPUI.md) for the full brief and
implementation plan.

## Layout

```
Cargo.toml                 virtual workspace (version and shared dependencies)
crates/dupreport-core/     domain logic, no GUI dependency
crates/mydupfinder-ui/     GPUI application (binary: mydupfinder-ui)
testdata/                  real report fixtures (CRLF line endings, keep byte-exact)
assets/icons/              application icons
docs/                      design brief
```

## Prerequisites

- Rust via [rustup](https://rustup.rs/); `rust-toolchain.toml` selects the
  right version (Rust 1.99, with rustfmt and clippy).
- **Windows:** Visual Studio Build Tools (MSVC, "Desktop development with C++" workload) and a Windows SDK.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Linux (Debian/Ubuntu):**

  ```sh
  sudo apt-get install -y clang pkg-config \
    libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
    libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev \
    libssl-dev libasound2-dev
  ```

## Commands

```sh
cargo build --workspace                              # build everything
cargo run -p mydupfinder-ui                          # run the app
cargo test --workspace                               # run all tests
cargo fmt --all                                      # format (CI uses -- --check)
cargo clippy --workspace --all-targets -- -D warnings
```

The first build compiles GPUI and takes several minutes.

## License

MIT, see [LICENSE](LICENSE).
