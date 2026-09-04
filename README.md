# ⚡ PrefixPug

<div align="center">

![PrefixPug Hero Banner](assets/hero_banner.jpg)

**A safe Steam/Proton `compatdata` and shader cache cleaner written in Rust.**

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)
[![Safety: SAFETY.md](https://img.shields.io/badge/Safety-Verified_Architecture-green.svg?style=for-the-badge)](SAFETY.md)
[![CI](https://img.shields.io/badge/CI-Passing-brightgreen.svg?style=for-the-badge&logo=github)](https://github.com/Bvaughan7/prefixpug/actions)
[![Release](https://img.shields.io/badge/Release-v0.2.1-blue.svg?style=for-the-badge&logo=github)](https://github.com/Bvaughan7/prefixpug/releases)

[The Safety Model](#-why-prefixpug-the-safety-model) •
[Interactive TUI](#-interactive-synthwave-tui) •
[CLI & Audit Mode](#-cli-commands--scripting) •
[Installation](#-installation) •
[Testing & Verification](#-testing--safe-sandbox) •
[Full Safety Spec (SAFETY.md)](SAFETY.md)

</div>

---

## 🛡 Why PrefixPug? (The Safety Model)

When you uninstall a Steam game on Linux, Valve's client leaves behind its Proton Wine prefix (`compatdata`) and compiled graphics pipelines (`shadercache`). Over time, this accumulates single-digit to low-double-digit gigabytes of abandoned files.

Writing a naive script to delete `compatdata/` folders with no matching `appmanifest_*.acf` is easy. What is difficult—and where naive scripts cause catastrophic data loss—is handling the edge cases:

1. **Non-Steam Shortcuts (`shortcuts.vdf`):** Games like *Battle.net*, *Epic Games Store*, emulators, and standalone launchers have full Proton prefixes in `compatdata`, but **never** have an `appmanifest_*.acf`. Naive cleaners destroy them. PrefixPug parses binary `shortcuts.vdf` across all user profiles to protect them.
2. **Unmounted Multi-Library Drives:** If a secondary NVMe or external SSD is disconnected, naive cleaners treat it as empty and delete all corresponding prefixes. PrefixPug verifies all configured drives and **aborts immediately** if any library is unreachable.
3. **Blocklist Save Vaulting (The Pug's Nose):** Games save in extensionless files, `.json`, `.xml`, `.bin`, and custom engine formats. Cleaners using extension allowlists (`.sav`, `.ess`) report "saves backed up" while silently destroying the actual save. PrefixPug archives the entire save root minus crash dumps and logs.
4. **Fsync & Verification Before Unlink:** Save vaults are cryptographically hashed (SHA-256), compressed, and `fsync`'d to physical storage before any prefix directory is unlinked.
5. **Symlink Escape Protection:** PrefixPug strictly ignores symlinks that resolve outside the prefix to protect your `$HOME` directory.

Read the complete engineering defense specification in [**`SAFETY.md`**](SAFETY.md).

---

## 🎨 Interactive Synthwave TUI

PrefixPug defaults to an interactive terminal dashboard built with `ratatui`:

<div align="center">

![PrefixPug Interactive TUI Demo](assets/prefixpug_demo.gif)

</div>

### Controls & Keybindings

| Key | Action | Description |
| :--- | :--- | :--- |
| `↑` / `k` | **Navigate Up** | Move cursor up through the prefix list |
| `↓` / `j` | **Navigate Down** | Move cursor down through the list |
| `Space` | **Toggle Selection** | Select or deselect highlighted prefix `[■]` |
| `a` | **Select All** | Batch select or deselect all visible deletable prefixes |
| `i` | **Invert Selection** | Flip selection across visible prefixes |
| `s` | **Cycle Sort Mode** | Sort by Size (descending), Age (oldest first), or AppID |
| `m` | **Toggle Mascot** | Toggle between data-dense Inspector and Cyberpug mascot |
| `/` | **Search / Filter** | Real-time interactive filter by AppID or game title |
| `c` | **Clean Selected** | Open the safety confirmation modal to vault and purge |
| `?` / `h` | **Help Matrix** | Toggle control help dialog |
| `q` / `Esc` | **Quit** | Close modal or exit application |

---

## 🚀 CLI Commands & Scripting

PrefixPug is safe-by-default: destructive CLI commands run in **dry-run mode** unless explicitly confirmed.

```bash
# Launch interactive TUI dashboard (default)
prefixpug

# Read-only audit of ALL prefixes (installed, non-Steam shortcuts, runtimes, orphans)
prefixpug audit

# Fast terminal scan of orphaned prefixes
prefixpug scan

# Structured JSON output for scripting and monitoring
prefixpug scan --json

# Safe simulation (safe default - no files modified)
prefixpug clean

# Execute non-interactive cleanup (requires --yes or --purge)
prefixpug clean --yes

# Clean only prefixes untouched for over 60 days
prefixpug clean --older-than 60 --yes

# Low-risk mode: Clean only shader caches without touching compatdata prefixes
prefixpug shaders --yes

# Filter installed games with prefixes untouched for over 90 days
prefixpug audit --stale

# Archive save files for a specific game/prefix without deleting anything
prefixpug vault 2141910

# List all archived save vaults
prefixpug backups

# Cryptographically verify a save vault against its SHA-256 manifest
prefixpug verify-backup <BACKUP_ID>

# Restore save files from an archive
prefixpug restore <BACKUP_ID> --target ~/RestoredSaves/
```

### 🎮 Steam Deck & Decky Loader
PrefixPug includes a native SteamOS [Decky Loader plugin](decky-plugin/) for one-tap Quick Access Menu (QAM) scanning, save vaulting, and shader cache cleanup on the Steam Deck.

---

## ⚡ Installation

### From Source
```bash
git clone https://github.com/Bvaughan7/prefixpug.git
cd prefixpug
./install.sh
```
The installer compiles the binary with Link-Time Optimization (`lto = true`) and installs it to `~/.local/bin/prefixpug` alongside shell autocompletions.

### Standalone Static Binaries
Download precompiled, statically linked binaries from [GitHub Releases](https://github.com/Bvaughan7/prefixpug/releases/latest):
- `prefixpug-x86_64-unknown-linux-musl.tar.gz` (Zero external dependencies)
- `prefixpug-x86_64-unknown-linux-gnu.tar.gz`

---

## 🧪 Testing & Safe Sandbox

PrefixPug includes an end-to-end mock Steam sandbox generator to safely verify orphan detection, non-Steam shortcut protection, and save file restoration:

```bash
# 1. Generate an isolated sandbox in /tmp
./tests/test_sandbox.sh

# 2. Run scan against the mock sandbox
prefixpug --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf scan

# 3. Launch the interactive TUI against the mock sandbox
prefixpug --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf

# 4. Run the full cargo test suite
cargo test
```

---

## ⚖ Transparency & License

- **AI Disclosure:** Developed with AI assistance; all logic, safety boundaries, and edge cases are verified by comprehensive unit and integration tests (see `tests/` and `SAFETY.md`).
- **License:** Licensed under the [MIT License](LICENSE). Copyright (c) 2026 Bryan Vaughan.
