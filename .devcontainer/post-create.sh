#!/usr/bin/env bash
# Runs once after the dev container is created.
set -euo pipefail

# Fresh named volumes are created owned by root; hand them to the dev user.
# Only the top-level directories: the shared Claude/gh volumes may already hold
# files written by other dev containers (same `vscode` uid 1000).
sudo chown vscode:vscode \
  "$HOME/.claude" \
  "$HOME/.config/gh" \
  "${PWD}/target" \
  /usr/local/cargo/registry \
  /usr/local/cargo/git

# Toolchain + components pinned in rust-toolchain.toml (same as CI).
rustup toolchain install

# Warm the crate cache so the first build does not have to.
cargo fetch --locked

echo
echo "Dev container ready:"
rustc --version
cargo --version
gh --version | head -n1
claude --version || true
echo
echo "Build & test:  cargo test --workspace"
echo "Run the app:   open the 'Desktop (noVNC)' port 6080 (password: vscode), then"
echo "               cargo run -p mydupfinder-ui -- \"testdata/Demo DiffFindDupsTheWholeLot dupReport.csv\""
