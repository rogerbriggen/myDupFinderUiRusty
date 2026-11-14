# Dev container image for myDupFinder UI (Rusty).
# Ubuntu 24.04 matches the `ubuntu-latest` CI runner. Rust, Node.js, GitHub CLI,
# Claude Code and the virtual desktop come from the features in devcontainer.json.
# Codex CLI and the Claude Code plugin are installed by post-create.sh.
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# GPUI build dependencies (same list as the Linux job in .github/workflows/ci.yml)
# plus what is needed to *run* the app inside the container:
#   mesa-vulkan-drivers  - lavapipe, a software Vulkan driver (no GPU passthrough needed)
#   libvulkan1, libgl1   - Vulkan loader / GL fallback
#   xdg-desktop-portal*  - backend for the "Open report" file dialog
#   fonts-dejavu-core    - a default font set for font-kit
RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        clang pkg-config \
        python3 python3-pip python3-venv python-is-python3 \
        ripgrep jq \
        libxkbcommon-dev libxkbcommon-x11-dev \
        libwayland-dev libvulkan-dev \
        libx11-xcb-dev libxcb1-dev \
        libfontconfig-dev libfreetype-dev \
        libssl-dev libasound2-dev \
        mesa-vulkan-drivers libvulkan1 libgl1 vulkan-tools \
        xdg-desktop-portal xdg-desktop-portal-gtk \
        fonts-dejavu-core \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Mount points for the named volumes in devcontainer.json. Creating them here
# owned by `vscode` means a fresh volume is initialised with that ownership.
RUN mkdir -p /home/vscode/.claude /home/vscode/.codex /home/vscode/.config/gh \
    && chown -R vscode:vscode /home/vscode/.claude /home/vscode/.codex /home/vscode/.config
