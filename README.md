# PrefixPug

<div align="center">

![PrefixPug Banner](assets/hero_banner.jpg)

**Safely reclaim 20–100 GB of storage from abandoned Proton prefixes and shader caches on Linux and Steam Deck without risking save data.**

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)
[![Safety: SAFETY.md](https://img.shields.io/badge/Safety-Verified_Architecture-green.svg?style=for-the-badge)](SAFETY.md)
[![CI](https://img.shields.io/badge/CI-Passing-brightgreen.svg?style=for-the-badge&logo=github)](https://github.com/Bvaughan7/prefixpug/actions)
[![Release](https://img.shields.io/badge/Release-v0.2.1-blue.svg?style=for-the-badge&logo=github)](https://github.com/Bvaughan7/prefixpug/releases)
[![Website](https://img.shields.io/badge/Website-Online-cyan.svg?style=for-the-badge)](https://bvaughan7.github.io/prefixpug/)

[Overview](#overview) •
[Quick Start](#quick-start) •
[Safety Architecture](#safety-architecture--threat-model) •
[Interactive TUI](#interactive-tui) •
[CLI Commands](#cli-commands--scripting) •
[Steam Deck](#steam-deck--decky-loader) •
[Installation](#installation) •
[Testing & Verification](#testing--verification) •
[SAFETY.md](SAFETY.md)

</div>

---

## Overview

If you game on **Linux** or the **Steam Deck**, your storage is quietly disappearing.

### 1. What is a Proton prefix (`compatdata`)?
Whenever you play a Windows game on Steam, Steam uses Proton to create a simulated Windows environment (stored in `~/.local/share/Steam/steamapps/compatdata/<AppID>/`). This directory contains a virtual `C:\` drive, registry hives, configurations, and graphics shader caches (`shadercache/<AppID>/`).

### 2. The Storage Accumulation
When you uninstall a game from Steam, **Steam deletes the game install files, but leaves the entire Windows prefix and shader cache behind.**
* After uninstalling 10 to 20 games, these leftover folders can silently consume **20 GB to 100+ GB** of your NVMe SSD or microSD card.

### 3. Why Not Just Delete Them with a Simple Script?
Because **many games store save files, offline progression, and configuration inside that prefix**:
* **Save Loss Risk:** Games lacking Steam Cloud support (or when offline) store saves under `drive_c/users/steamuser/`. Deleting the folder deletes those saves permanently.
* **Non-Steam Games Risk:** Games added manually (Battle.net, Heroic Games Launcher, GOG, emulators) do not have Steam `appmanifest_*.acf` files. Naive cleanup scripts mistake them for abandoned folders and delete your active prefixes.
* **Disconnected Storage Risk:** If a secondary SSD or microSD card is unplugged, basic scripts assume all games on that drive are uninstalled and wipe their prefixes from your primary drive.

### 4. How PrefixPug Solves This Safely
PrefixPug implements safety-first invariants before modifying anything on disk:
1. **Pre-Deletion Save Archiving:** Before deleting any orphaned prefix, PrefixPug scans user directories (`Saved Games`, `Documents`, `AppData`, etc.) and compresses detected saves into `~/.local/share/prefixpug/backups/` alongside a cryptographic SHA-256 manifest.
2. **Non-Steam Shortcut Protection:** Ingests Steam's binary `shortcuts.vdf` across all user profiles so manual shortcuts and third-party launchers are recognized and protected.
3. **Mount Point Invariant:** Aborts immediately if any configured Steam library storage mount is disconnected.
4. **Advisory Process Locking:** Verifies Steam and Proton processes aren't currently running or writing to prefixes.
5. **Direct Restoration:** Any archived save can be extracted at any time with a single command: `prefixpug restore <BACKUP_ID> --target ~/RestoredSaves/`.

---

## Quick Start

### 1. Launch the Interactive Dashboard
```bash
prefixpug
```
*(On Steam Deck, you can also use our [Decky Loader plugin](#steam-deck--decky-loader) directly in Game Mode!)*

### 2. Review Detected Prefixes
PrefixPug scans all mounted Steam libraries in seconds:
* Identifies uninstalled games with leftover prefixes or shader caches.
* Reports physical disk usage (accounting for sparse files).
* Flags whether local save files are present.

### 3. Clean and Reclaim
* Use arrow keys or `j`/`k` to navigate.
* Press `Space` to toggle individual items, or `a` to toggle all.
* Press `c` to clean: PrefixPug archives all detected saves first to the local backup directory, then safely unlinks the orphaned prefix and shader cache directories.

---

## Safety Architecture & Threat Model

Writing a naive script to delete `compatdata/` folders without matching `appmanifest_*.acf` files is straightforward, but prone to catastrophic edge cases. PrefixPug enforces defensive invariants validated by automated integration tests.

| Scenario | Naive Shell Script (`rm -rf`) | PrefixPug | Validating Test |
|:---|:---|:---|:---|
| **Disconnected Drive / SD Card** | Misclassifies games as orphans and deletes live prefixes | Verifies all configured libraries; halts immediately with Exit Code 2 | [`test_abort_on_unmounted_library`](tests/integration_tests.rs) |
| **Non-Steam Shortcuts (Battle.net, Heroic, emulators)** | Destroys custom prefixes (no `appmanifest`) | Ingests binary `shortcuts.vdf` across all user profiles to protect them | [`test_non_steam_shortcut_protection`](tests/integration_tests.rs) |
| **Local Saves (`.json`, `.xml`, extensionless blobs)** | Silent, irreversible data loss | Directory-root save detection archived to SHA-256 fsynced archive | [`test_extensionless_canary_save_survival`](tests/integration_tests.rs) |
| **Wine Symlinks Escaping Prefix** | Risks traversing and unlinking personal files in `$HOME` | Strict path traversal jail; never unlinks outside designated prefixes | [`test_symlink_traversal_refusal`](tests/integration_tests.rs) |
| **Steam Infrastructure Runtimes** | Deletes Proton/Soldier/Sniper shared runtimes | Deny-lists all standard Steam runtime and compatibility AppIDs | [`test_runtime_deny_list_protection`](tests/integration_tests.rs) |
| **Archive Corruption / Partial Writes** | Broken backups lead to unrecoverable data loss | Manifest SHA-256 verification and atomic fsync write before deletion | [`test_backup_manifest_sha256_verification`](tests/integration_tests.rs) |
| **Sparse Files & Apparent Size** | Inaccurate storage reclaim estimates | Queries physical filesystem blocks (`st_blocks * 512`) | Unit test suite |

### Core Architectural Invariants
1. **Multi-Library Mount Guard:** If any drive or microSD card configured in `libraryfolders.vdf` is unmounted, PrefixPug halts immediately rather than misclassifying games on that drive as orphans. (*Test: `test_abort_on_unmounted_library`*)
2. **Non-Steam Shortcut Protection:** Ingests Steam's binary `shortcuts.vdf` across all user profiles, computing 32-bit CRC IDs to protect custom launchers, emulators, and manual shortcuts lacking standard Steam ACF manifests. (*Test: `test_non_steam_shortcut_protection`*)
3. **Steam Infrastructure Deny-List:** Critical Steam runtimes (Steam Linux Runtime soldier/sniper/scout/medic, Proton 3.7–9.0, Proton Experimental, Proton Hotfix, SteamVR, EAC, and BattlEye) are hard-coded in an infrastructure deny-list and never targeted for removal. (*Test: `test_runtime_deny_list_protection`*)
4. **Directory-Root Save Detection Engine:** Rather than relying on fragile extension allowlists, PrefixPug traverses entire save roots (`Saved Games`, `Documents`, `AppData`) while filtering out transient noise (crash dumps, CEF/browser caches). This reliably preserves extensionless save blobs, SQLite databases, `.json`, and `.xml` saves. (*Test: `test_extensionless_canary_save_survival`*)
5. **Path Traversal & Symlink Jail:** All operations canonicalize paths and refuse to follow symlinks out of the prefix root, ensuring files in `$HOME` or system paths are never touched. (*Test: `test_symlink_traversal_refusal`*)
6. **Cryptographic Verification & fsync:** Every save archive is compressed (`.tar.gz`), audited with per-file SHA-256 checksums in `manifest.json`, and flushed to disk with `fsync` before any prefix directory is unlinked. (*Test: `test_backup_manifest_sha256_verification`*)
7. **Process & Concurrency Guard:** Inspects `/proc` and checks non-blocking advisory file locks (`pfx.lock`) to prevent deleting prefixes while games or Steam are actively writing.

Read the full threat model specification in [**`SAFETY.md`**](SAFETY.md).

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
| `c` | **Clean Selected** | Open confirmation modal to archive saves and reclaim storage |
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

# List all archived save backups
prefixpug backups

# Cryptographically verify an archived save backup against its SHA-256 manifest
prefixpug verify-backup <BACKUP_ID>

# Restore save files from an archive
prefixpug restore <BACKUP_ID> --target ~/RestoredSaves/

# Generate shell completions (bash, zsh, fish, etc.)
prefixpug completions bash
```

---

## Steam Deck & Decky Loader

PrefixPug includes a native SteamOS [Decky Loader plugin](decky-plugin/) with a React/TypeScript Quick Access Menu (QAM) interface and an asynchronous Python RPC bridge. It enables one-tap prefix scanning, save file archiving, and shader cache cleanup directly inside Steam Game Mode.

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

## Testing & Verification

PrefixPug maintains an automated test suite covering unit operations and end-to-end integration safety scenarios:

```bash
# 1. Run all unit and integration tests (29 tests)
cargo test --all-targets

# 2. Run the integration test suite specifically
cargo test --test integration_tests

# 3. Generate an isolated sandbox in /tmp for manual inspection
./tests/test_sandbox.sh

# 4. Run scan against the mock sandbox
prefixpug --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf scan

# 5. Launch the interactive TUI against the mock sandbox
prefixpug --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf
```

---

## Transparency & License

* **AI Disclosure:** Developed with AI assistance; all logic, safety boundaries, and edge cases are verified by comprehensive unit and integration tests (see `tests/` and `SAFETY.md`).
* **License:** Licensed under the [MIT License](LICENSE). Copyright (c) 2026 Bryan Vaughan.
