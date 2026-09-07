# PrefixPug

<div align="center">

![PrefixPug Banner](assets/hero_banner.jpg)

**A safe, high-performance Steam/Proton `compatdata` and shader cache cleaner written in Rust.**

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)
[![Safety: SAFETY.md](https://img.shields.io/badge/Safety-Verified_Architecture-green.svg?style=for-the-badge)](SAFETY.md)
[![CI](https://img.shields.io/badge/CI-Passing-brightgreen.svg?style=for-the-badge&logo=github)](https://github.com/Bvaughan7/prefixpug/actions)
[![Release](https://img.shields.io/badge/Release-v0.2.1-blue.svg?style=for-the-badge&logo=github)](https://github.com/Bvaughan7/prefixpug/releases)
[![Website](https://img.shields.io/badge/Website-Online-cyan.svg?style=for-the-badge)](https://bvaughan7.github.io/prefixpug/)

[Website](https://bvaughan7.github.io/prefixpug/) •
[What Is This? (ELI5)](#what-is-this-the-problem-explained-simply) •
[Quick Start](#quick-start-in-30-seconds) •
[Safety Model](#why-prefixpug-the-safety-model) •
[Interactive TUI](#interactive-tui) •
[CLI & Scripting](#cli-commands--scripting) •
[Steam Deck](#steam-deck--decky-loader) •
[Installation](#installation) •
[Safety Spec (SAFETY.md)](SAFETY.md)

</div>

---

## What Is This? (The Problem Explained Simply)

If you game on **Linux** or the **Steam Deck**, your storage is quietly disappearing. Here is why:

### 1. What is a Proton prefix (`compatdata`)?
Whenever you play a Windows game on Steam, Steam uses Proton to create a simulated Windows directory (called a **prefix**, stored in `compatdata`). This folder contains a virtual `C:\` drive, Windows registries, configuration files, and graphics shader caches (`shadercache`).

### 2. The Storage Leak
When you uninstall a game from Steam, **Steam deletes the game files, but leaves the entire Windows prefix and shader cache behind forever.** 
* After uninstalling 10 to 20 games, these leftover "ghost" folders can silently consume **20 GB, 50 GB, or over 100 GB** of your NVMe SSD or microSD card.

### 3. Why Not Just Delete Them with a Script or Simple Cleaner?
Because **many games store your actual save files, mods, and configurations inside that prefix!**
* **Save Loss Danger:** If a game doesn't support Steam Cloud (or if cloud sync failed), deleting that folder deletes your save game permanently.
* **Custom Games Danger:** Games you added yourself (like *Battle.net*, *Heroic Games Launcher*, *GOG Galaxy*, or emulators) don't have Steam manifest files. Naive cleanup scripts mistake them for abandoned folders and wipe your installed non-Steam games!
* **Disconnected Drives:** If your external SSD or secondary game drive is unplugged, basic scripts assume all those games are uninstalled and wipe their prefixes from your main drive!

### 4. How PrefixPug Solves This
PrefixPug is built with **Safety Above All** as its prime directive:
1. 🐕 **The Pug's Nose (Automatic Save Vault):** Before deleting any leftover folder, PrefixPug automatically searches it for save files, documents, and game configs. It archives and cryptographically verifies them into a safe vault (`~/.local/share/prefixpug/backups/`).
2. 🎮 **Protects Custom Games:** It reads Steam's binary shortcut records so your *Battle.net*, *Heroic*, and emulator prefixes are recognized and never deleted.
3. 🛑 **Drive Mount Guard:** If any configured drive is disconnected or unmounted, PrefixPug halts immediately instead of making dangerous assumptions.
4. ♻️ **Easy One-Command Restore:** If you ever want your save files back, restoring them takes a single command: `prefixpug restore <ID>`.

---

## Quick Start (In 30 Seconds)

You don't need to know any complicated terminal commands to use PrefixPug:

### 1. Launch the Friendly Dashboard
Open your terminal and run:
```bash
prefixpug
```
*(On Steam Deck, you can also use our [Decky Loader plugin](#steam-deck--decky-loader) right from the Quick Access Menu!)*

### 2. Review Leftover Games
PrefixPug will scan your drives in seconds and display an interactive dashboard:
* See which uninstalled games left folders behind.
* See exactly how much storage you'll get back.
* See whether each game has save files or Steam Cloud copies.

### 3. Clean and Reclaim
* Use your arrow keys (or `j`/`k`) to browse.
* Press `Space` to select/deselect, or `a` to select all.
* Press `c` to clean! PrefixPug will vault all your save files safely first, then reclaim your disk space.

---

## Why PrefixPug? (The Safety Model)

Writing a naive script to delete `compatdata/` folders with no matching `appmanifest_*.acf` is easy. What is difficult—and where naive cleaners cause catastrophic data loss—is handling real-world edge cases.

| Scenario | Naive Shell Script (`rm -rf`) | PrefixPug |
|:---|:---|:---|
| **Disconnected Drive / SD Card** | Misclassifies games as orphans and deletes live prefixes | Verifies all configured libraries; halts immediately with Exit Code 2 |
| **Non-Steam Shortcuts (Battle.net, Heroic, emulators)** | Destroys custom prefixes (no `appmanifest`) | Ingests binary `shortcuts.vdf` across all user profiles to protect them |
| **Local Saves (`.json`, `.xml`, extensionless blobs)** | Silent, irreversible data loss | Whole-root save vaulting to SHA-256 fsynced archive before removal |
| **Steam Running Concurrently** | Deletes files during game writes or downloads | Detects active Steam processes and lock files, aborting safely |
| **Wine Symlinks Escaping Prefix** | Risks traversing and unlinking personal files in `$HOME` | Strict path traversal jail; never traverses outside designated prefixes |
| **Sparse Files & Allocation** | Wildly inaccurate apparent file size reports | Queries physical filesystem blocks (`st_blocks * 512`) and `statvfs` deltas |

### Core Architectural Defenses
1. **Multi-Library Mount Guard (P0-1):** If a secondary NVMe, external SSD, or Steam Deck microSD configured in `libraryfolders.vdf` is unmounted, PrefixPug aborts immediately rather than misclassifying games on that drive as orphans.
2. **Non-Steam Shortcut Parser (P0-2):** Ingests Steam's binary `shortcuts.vdf` across all user profiles, computing 32-bit CRC IDs and permanently protecting custom launchers and emulator prefixes.
3. **Steam Infrastructure Deny-List (P0-3):** Critical Steam runtimes (Steam Linux Runtime soldier/sniper/scout/medic, Proton 3.7–9.0, Proton Experimental, Proton Hotfix, SteamVR, EAC, and BattlEye) are hard-locked from cleanup.
4. **Blocklist Save Engine (P0-4):** Inverts traditional extension allowlists. Archives entire user directories (`Saved Games`, `Documents`, `AppData`) minus crash dumps and browser caches, safely capturing extensionless, `.json`, and `.xml` saves.
5. **Cryptographic Verification & fsync (P1-5):** Every save archive is compressed, audited with per-file SHA-256 checksums in `manifest.json`, and flushed with `fsync` before any prefix directory is unlinked.
6. **Process & Concurrency Guard (P0-6):** Inspects `/proc` and tests non-blocking advisory file locks (`pfx.lock`) to prevent deleting prefixes while games or Steam are actively writing.

Read the full technical specification in [**`SAFETY.md`**](SAFETY.md).

---

## Interactive TUI

PrefixPug includes an interactive terminal user interface built with `ratatui`:

<div align="center">

![PrefixPug Interactive TUI Demo](assets/prefixpug_tui_clean.gif)

</div>

### Controls & Keybindings

| Key | Action | Description |
|:---|:---|:---|
| `↑` / `k` | **Navigate Up** | Move cursor up through the prefix list |
| `↓` / `j` | **Navigate Down** | Move cursor down through the list |
| `Space` | **Toggle Selection** | Select or deselect highlighted prefix `[■]` |
| `a` | **Select All** | Batch select or deselect all visible deletable prefixes |
| `i` | **Invert Selection** | Flip selection across all visible prefixes |
| `s` | **Cycle Sort Mode** | Sort by Size (descending), Age (oldest first), or AppID |
| `/` | **Search / Filter** | Filter prefixes in real-time by AppID or game title |
| `c` | **Clean Selected** | Open confirmation modal to vault saves and reclaim storage |
| `?` / `h` | **Help Dialog** | Toggle on-screen keybinding reference |
| `q` / `Esc` | **Quit** | Close modal or exit application |

---

## CLI Commands & Scripting

PrefixPug is safe-by-default: destructive CLI operations run in **dry-run mode** unless explicitly confirmed. It provides structured JSON output for automations, monitoring, and headless servers.

```bash
# Launch interactive TUI dashboard (default)
prefixpug

# Read-only audit of ALL prefixes (installed, non-Steam shortcuts, runtimes, orphans)
prefixpug audit

# Filter installed games with prefixes untouched for over 90 days
prefixpug audit --stale

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

# Archive save files for a specific game/prefix without deleting anything
prefixpug vault 2141910

# List all archived save vaults
prefixpug backups

# Cryptographically verify a save vault against its SHA-256 manifest
prefixpug verify-backup <BACKUP_ID>

# Restore save files from an archive
prefixpug restore <BACKUP_ID> --target ~/RestoredSaves/

# Generate shell completions (bash, zsh, fish, etc.)
prefixpug completions bash
```

---

## Steam Deck & Decky Loader

PrefixPug includes a native SteamOS [Decky Loader plugin](decky-plugin/) with a React/TypeScript Quick Access Menu (QAM) interface and an asynchronous Python RPC bridge. It enables one-tap prefix scanning, save vaulting, and shader cache cleanup directly inside Steam Game Mode.

See the [Decky Plugin Guide](decky-plugin/README.md) for installation details.

---

## Installation

### One-Line Install Script
```bash
curl -sSL https://raw.githubusercontent.com/Bvaughan7/prefixpug/main/install.sh | bash
```
This automatically detects your system architecture, installs the statically linked binary to `~/.local/bin/prefixpug`, and configures shell completions and manual pages.

### From Source
```bash
git clone https://github.com/Bvaughan7/prefixpug.git
cd prefixpug
cargo build --release
install -m 755 target/release/prefixpug ~/.local/bin/prefixpug
```

### Standalone Static Binaries
Download precompiled, statically linked binaries from [GitHub Releases](https://github.com/Bvaughan7/prefixpug/releases/latest):
* `prefixpug-x86_64-unknown-linux-musl.tar.gz` (Zero external dependencies)
* `prefixpug-x86_64-unknown-linux-gnu.tar.gz`

### Arch Linux / SteamOS (AUR / PKGBUILD)
A ready-to-use [`PKGBUILD`](packaging/PKGBUILD) is included in the `packaging/` directory.

---

## Testing & Safe Sandbox

PrefixPug includes an end-to-end mock Steam sandbox generator to safely verify orphan detection, non-Steam shortcut protection, and save file restoration without touching your real Steam library:

```bash
# 1. Generate an isolated sandbox in /tmp
./tests/test_sandbox.sh

# 2. Run scan against the mock sandbox
prefixpug --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf scan

# 3. Launch the interactive TUI against the mock sandbox
prefixpug --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf

# 4. Run the full cargo test suite
cargo test --all-targets
```

---

## Transparency & License

* **AI Disclosure:** Developed with AI assistance; all logic, safety boundaries, and edge cases are verified by comprehensive unit and integration tests (see `tests/` and `SAFETY.md`).
* **License:** Licensed under the [MIT License](LICENSE). Copyright (c) 2026 Bryan Vaughan.
