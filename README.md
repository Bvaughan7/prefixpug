# ⚡ PrefixPug

<div align="center">

```text
       ┌──────────────────────┐
  /\_/\│  CYBERPUG NEON M-01  │
 ( •ᴥ•)│  *sniff* ~ ~ *snort* │
  /   \│  Archiving saves before byte purge
 ( " " )
  └───────────────────────────┘
```

**High-performance Rust utility to sniff out and safely reclaim orphaned Steam/Proton `compatdata` and shader caches.**

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/Bvaughan7/prefixpug/actions/workflows/ci.yml/badge.svg)](https://github.com/Bvaughan7/prefixpug/actions/workflows/ci.yml)

</div>

---

## 🐾 Overview

**PrefixPug** is a fast, safe, and aesthetic terminal tool designed to reclaim gigabytes of NVMe/SSD storage by identifying orphaned Wine/Proton prefixes (`compatdata`) and shader caches left behind after uninstalling Steam games.

### 🛡 The Pug's Nose (Save Vault & Safety)
- **Automatic Save Heuristics:** Scans through orphaned prefixes for local save files (`Saved Games/`, `Documents/My Games/`, `AppData/`, `.sav`, `.ess`, etc.).
- **Gzip Vault Backups:** Automatically compresses detected save files into `.tar.gz` archives with detailed `manifest.json` metadata in `~/.local/share/prefixpug/backups/`.
- **One-Command Restore:** Restore your saved games anytime via `prefixpug restore <BACKUP_ID>`.
- **Safety First:** Strict confirmation dialogs prior to any filesystem modification or deletion. Zero unwraps; 100% safe `Result`-driven error handling.

---

## 🎨 Cyberpunk Synthwave TUI

PrefixPug comes with a terminal dashboard built with `ratatui` featuring an 80s Synthwave / Cyberpunk aesthetic:

- **3-Pane Dynamic Interface:**
  - **Left Pane:** Orphaned Prefixes table with checkboxes, AppIDs, inferred game titles, sizes, and save count badges.
  - **Top-Right Pane:** Animated Cyberpug mascot with animated wireframe border and live sniffing cycle.
  - **Bottom-Right Pane:** Deep inspector with detailed breakdown of compatdata, shadercache, and individual detected save paths.
  - **Bottom Bar:** Visual space-reclamation progress gauge and keyboard shortcut guide.
- **Interactive Controls:**
  - `↑` / `k` — Navigate up
  - `↓` / `j` — Navigate down
  - `Space` — Toggle selection for current prefix
  - `a` — Select all / deselect all
  - `i` — Invert selection
  - `/` — Live search and filter by AppID or game name
  - `c` — Clean selected prefixes (opens confirmation modal)
  - `?` / `h` — Open Cyberpunk Help & Control matrix
  - `q` / `Esc` — Quit application

---

## 🚀 CLI Commands & Automation

PrefixPug can also run non-interactively or headlessly in shell scripts and automated pipelines:

```bash
# Launch interactive Cyberpunk TUI (default)
prefixpug

# Scan and print raw list of orphaned prefixes
prefixpug scan

# Output scan results as JSON (for scripting/jq)
prefixpug scan --json

# Filter scan to specific AppIDs
prefixpug scan --appids 123456,789012

# Dry run cleanup (safe simulation)
prefixpug clean --dry-run

# Clean specific AppIDs with confirmation prompt
prefixpug clean --appids 123456

# Non-interactive automated clean
prefixpug clean --auto-clean

# List all archived save vaults
prefixpug backups

# Restore a save backup to a target directory
prefixpug restore 123456_1787888781 --target ~/RestoredSaves/
```

---

## 🏗 Architecture

```
prefixpug/
├── Cargo.toml
├── src/
│   ├── lib.rs           (Library exports)
│   ├── main.rs          (CLI entrypoint & event loops)
│   ├── cli.rs           (Clap command and option definitions)
│   ├── scanner.rs       (Filesystem traversal, size calculation, save sniffing)
│   ├── vdf_parser.rs    (VDF/ACF parser, library discovery, title inference)
│   ├── backup.rs        (Gzip save vaulting, manifests, and restore logic)
│   └── tui/
│       ├── mod.rs
│       ├── app.rs       (State management, filtering, animations)
│       └── ui.rs        (Ratatui widgets, Synthwave styling, mascot)
└── tests/
    └── integration_tests.rs (End-to-end Steam library mock tests)
```

---

## 🧪 Testing

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
