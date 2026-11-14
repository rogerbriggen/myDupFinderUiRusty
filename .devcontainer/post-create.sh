#!/usr/bin/env bash
# Runs once after the dev container is created.
set -euo pipefail

export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"

# Fresh named volumes are created owned by root; hand them to the dev user.
# Only the top-level directories: the shared Claude/Codex/gh volumes may already hold
# files written by other dev containers (same `vscode` uid 1000).
sudo chown vscode:vscode \
  "$HOME/.claude" \
  "$CODEX_HOME" \
  "$HOME/.config/gh" \
  "${PWD}/target" \
  /usr/local/cargo/registry \
  /usr/local/cargo/git

# Keep credentials in the shared volume rather than a container-local keyring.
# Seed fresh configurations only; existing shared settings belong to the user.
if [[ ! -f "$CODEX_HOME/config.toml" ]]; then
  (umask 077; printf '%s\n' 'cli_auth_credentials_store = "file"' > "$CODEX_HOME/config.toml")
fi

# Node is provided by the Node feature, with a user-writable global npm prefix.
npm install --global @openai/codex

# Install at user scope so the plugin lives in the shared Claude volume.
# Listing first makes this safe to rerun when another container added it already.
codex_marketplaces="$(claude plugin marketplace list --json)"
if ! jq -e 'any(.[]; .name == "openai-codex")' <<< "$codex_marketplaces" > /dev/null; then
  claude plugin marketplace add openai/codex-plugin-cc
fi
claude plugin install codex@openai-codex --scope user

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
codex --version
python3 --version
echo
echo "Build & test:  cargo test --workspace"
echo "Codex login:  codex login --device-auth (shared via the codex-config volume)"
echo "Run the app:   open the 'Desktop (noVNC)' port 6080 (password: vscode), then"
echo "               cargo run -p mydupfinder-ui -- \"testdata/Demo DiffFindDupsTheWholeLot dupReport.csv\""
