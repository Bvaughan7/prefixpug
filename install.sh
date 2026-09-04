#!/usr/bin/env bash
set -euo pipefail

echo -e "\033[1;36mPREFIXPUG :: Installing PrefixPug locally...\033[0m"

INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "${INSTALL_DIR}"

if command -v cargo >/dev/null 2>&1 && [ -f "Cargo.toml" ]; then
    echo "Compiling optimized release binary from local source..."
    cargo build --release
    cp target/release/prefixpug "${INSTALL_DIR}/prefixpug"
else
    echo "Downloading precompiled statically linked binary (musl) from GitHub Releases..."
    curl -sSL "https://github.com/Bvaughan7/prefixpug/releases/latest/download/prefixpug-x86_64-unknown-linux-musl.tar.gz" | tar -xz -C "${INSTALL_DIR}"
fi
chmod +x "${INSTALL_DIR}/prefixpug"

# Shell completions
if [ -d "${HOME}/.local/share/bash-completion" ] || [ -d "/usr/share/bash-completion" ]; then
    mkdir -p "${HOME}/.local/share/bash-completion/completions"
    echo "Installing bash completions..."
    "${INSTALL_DIR}/prefixpug" completions bash > "${HOME}/.local/share/bash-completion/completions/prefixpug"
fi

if [ -d "${HOME}/.zsh" ] || [ -d "${HOME}/.zfunc" ]; then
    mkdir -p "${HOME}/.zfunc"
    echo "Installing zsh completions to ~/.zfunc/_prefixpug..."
    "${INSTALL_DIR}/prefixpug" completions zsh > "${HOME}/.zfunc/_prefixpug"
fi

if [ -d "${HOME}/.config/fish" ] || [ "$(basename "${SHELL:-}")" = "fish" ]; then
    mkdir -p "${HOME}/.config/fish/completions"
    echo "Installing fish completions..."
    "${INSTALL_DIR}/prefixpug" completions fish > "${HOME}/.config/fish/completions/prefixpug.fish"
fi

# Man page
if [ -f "man/prefixpug.1" ]; then
    MANDIR="${HOME}/.local/share/man/man1"
    mkdir -p "${MANDIR}"
    cp man/prefixpug.1 "${MANDIR}/prefixpug.1"
fi

echo -e "\033[1;32m✓ PrefixPug successfully installed to ${INSTALL_DIR}/prefixpug\033[0m"

if ! echo "${PATH}" | grep -q "${HOME}/.local/bin"; then
    echo -e "\033[1;33mNote: Make sure ~/.local/bin is in your PATH (e.g. export PATH=\"\$HOME/.local/bin:\$PATH\")\033[0m"
fi

echo -e "Run \033[1;36mprefixpug\033[0m or \033[1;36mprefixpug --help\033[0m to get started!"
