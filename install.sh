#!/usr/bin/env bash
set -euo pipefail

echo -e "\033[1;36mPREFIXPUG :: Installing PrefixPug locally...\033[0m"

INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "${INSTALL_DIR}"

ARCH="$(uname -m)"

if command -v cargo >/dev/null 2>&1 && [ -f "Cargo.toml" ]; then
    echo "Compiling optimized release binary from local source..."
    cargo build --release
    install -m 755 target/release/prefixpug "${INSTALL_DIR}/prefixpug"
else
    if [ "${ARCH}" != "x86_64" ]; then
        echo -e "\033[1;31mError: Prebuilt binaries are currently only available for x86_64 (detected ${ARCH}).\033[0m" >&2
        echo "Please install Rust and Cargo to compile from source: https://rustup.rs" >&2
        exit 1
    fi

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "${TMPDIR}"' EXIT

    echo "Downloading precompiled statically linked binary (musl) from GitHub Releases..."
    if ! curl -sSLf "https://github.com/Bvaughan7/prefixpug/releases/latest/download/prefixpug-x86_64-unknown-linux-musl.tar.gz" -o "${TMPDIR}/prefixpug.tar.gz"; then
        echo -e "\033[1;31mError: Failed to download release archive.\033[0m" >&2
        exit 1
    fi

    tar -xzf "${TMPDIR}/prefixpug.tar.gz" -C "${TMPDIR}"
    if [ ! -f "${TMPDIR}/prefixpug" ]; then
        echo -e "\033[1;31mError: Binary prefixpug was not found in release archive.\033[0m" >&2
        exit 1
    fi

    install -m 755 "${TMPDIR}/prefixpug" "${INSTALL_DIR}/prefixpug"
fi

# Shell completions (best-effort)
if [ -d "${HOME}/.local/share/bash-completion" ] || [ -d "/usr/share/bash-completion" ]; then
    mkdir -p "${HOME}/.local/share/bash-completion/completions"
    echo "Installing bash completions..."
    "${INSTALL_DIR}/prefixpug" completions bash > "${HOME}/.local/share/bash-completion/completions/prefixpug" 2>/dev/null || true
fi

if [ -d "${HOME}/.zsh" ] || [ -d "${HOME}/.zfunc" ]; then
    mkdir -p "${HOME}/.zfunc"
    echo "Installing zsh completions to ~/.zfunc/_prefixpug..."
    "${INSTALL_DIR}/prefixpug" completions zsh > "${HOME}/.zfunc/_prefixpug" 2>/dev/null || true
fi

if [ -d "${HOME}/.config/fish" ] || [ "$(basename "${SHELL:-}")" = "fish" ]; then
    mkdir -p "${HOME}/.config/fish/completions"
    echo "Installing fish completions..."
    "${INSTALL_DIR}/prefixpug" completions fish > "${HOME}/.config/fish/completions/prefixpug.fish" 2>/dev/null || true
fi

# Man page
if [ -f "man/prefixpug.1" ]; then
    MANDIR="${HOME}/.local/share/man/man1"
    mkdir -p "${MANDIR}"
    cp man/prefixpug.1 "${MANDIR}/prefixpug.1" 2>/dev/null || true
fi

echo -e "\033[1;32m✓ PrefixPug successfully installed to ${INSTALL_DIR}/prefixpug\033[0m"

if ! echo "${PATH}" | grep -q "${HOME}/.local/bin"; then
    echo -e "\033[1;33mNote: Make sure ~/.local/bin is in your PATH (e.g. export PATH=\"\$HOME/.local/bin:\$PATH\")\033[0m"
fi

echo -e "Run \033[1;36mprefixpug\033[0m or \033[1;36mprefixpug --help\033[0m to get started!"
