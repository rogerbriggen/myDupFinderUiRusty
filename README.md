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

### Dev container

`.devcontainer/` sets up everything in one Linux container (Ubuntu 24.04):
Rust 1.99 with rustfmt and clippy, the GPUI build dependencies, Node.js,
GitHub CLI (`gh`), Claude Code (`claude`) and a small X11 desktop to run the app.
Open the folder in VS Code and choose **Reopen in Container**. The first
start installs the toolchain and fetches all crates.

- **Build & test:** the commands below work unchanged.
- **Run the app:** open the forwarded port **6080** (*Desktop (noVNC)*) in a
  browser, password `vscode`, then
  `cargo run -p mydupfinder-ui -- "testdata/Demo DiffFindDupsTheWholeLot dupReport.csv"`.
  Rendering uses the Mesa software Vulkan driver (lavapipe): fine for
  checking the UI, not for judging performance.
- **Shared Claude folder:** `~/.claude` is the named Docker volume
  `claude-code-config`, and `CLAUDE_CONFIG_DIR` points there, so login,
  settings and history survive rebuilds. Any other dev container that mounts
  the same volume shares them:

  ```jsonc
  "mounts": ["source=claude-code-config,target=/home/vscode/.claude,type=volume"],
  "containerEnv": { "CLAUDE_CONFIG_DIR": "/home/vscode/.claude" }
  ```

  The `gh` login is shared the same way through the volume `gh-cli-config`
  (`/home/vscode/.config/gh`). Both expect the user `vscode` (uid 1000).
- `target/` and the cargo registry live in volumes, which keeps builds fast on
  Windows and macOS hosts.
- Open the container from a normal clone, not from a `git worktree`: a
  worktree's `.git` file points to a host path that does not exist inside the
  container.

## Commands

```sh
cargo build --workspace                              # build everything
cargo run -p mydupfinder-ui                          # run the app
cargo test --workspace                               # run all tests
cargo fmt --all                                      # format (CI uses -- --check)
cargo clippy --workspace --all-targets -- -D warnings
```

The first build compiles GPUI and takes several minutes.

## Packaging & releases

Installers are built with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager);
its configuration is in `crates/mydupfinder-ui/Cargo.toml`
(`[package.metadata.packager]`). The Windows executable embeds the icon and
version information through `build.rs` (`winresource`).

Build a package locally (on the OS you package for):

```sh
cargo install cargo-packager --locked --version 0.11.8
cargo packager --release -p mydupfinder-ui --formats nsis       # Windows: setup .exe
cargo packager --release -p mydupfinder-ui --formats deb,appimage  # Linux
cargo packager --release -p mydupfinder-ui --formats dmg        # macOS
```

The results land in `target/packager/`. cargo-packager downloads the tools it
needs (NSIS, linuxdeploy) itself.

CI (`.github/workflows/ci.yml`): outside pull requests, every OS whose test
job runs also gets a package job after it: Linux and Windows on pushes to
`main`, all three OSes on `v*` tags, and the ticked OSes on manual runs
(Actions tab, "Run workflow"). macOS is not packaged on `main`.
The installers are workflow artifacts named
`mydupfinder-ui-<os>-packages` (kept 14 days). Pull requests only run the tests.

**Release:** set the version in the workspace `Cargo.toml`, merge it, then push
a tag `vX.Y.Z` that matches it exactly (for example `git tag v0.1.0 && git push origin v0.1.0`).
The release job refuses a tag that differs from the Cargo version and otherwise
publishes a GitHub Release with the installers attached and generated notes.
The installers are not code-signed.

## Version

The SemVer lives only in the workspace `Cargo.toml` (`[workspace.package] version`).
`crates/mydupfinder-ui/build.rs` adds git information at build time by running
`git describe --tags --always --dirty` and `git rev-parse --short HEAD` (no extra
crates, no GitVersion tool). The app shows the result GitVersion-style as SemVer
build metadata, e.g. `0.1.0+1bb8187` (`0.1.0+1bb8187.dirty` with uncommitted
changes): in the window title and, with the full `git describe` string as
tooltip, at the right end of the toolbar.

Where git is not available (source tarball, container), set the environment
variable `MYDUPFINDER_BUILD_GIT` (for example to the short sha) when building; it
replaces the git values. If neither works the build still succeeds and the
version is shown without the `+...` part.

## License

MIT, see [LICENSE](LICENSE).
