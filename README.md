# ⚡ PrefixPug

<div align="center">

```text
       ┌──────────┐
  /\_/\│ CYBERPUG │
 ( •ᴥ•)│ *sniff*  │
  /   \│ Archiving saves before byte purge
 ( " " )
  └───────┘
```

**Sniff out and safely clean up orphaned Steam/Proton `compatdata` and shader caches.**

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

</div>

---

## 🐾 Overview

**PrefixPug** is a high-performance terminal utility written in Rust designed to reclaim NVMe/SSD storage by locating orphaned Wine/Proton prefixes (`compatdata`) and shader caches left behind after uninstalling Steam games.

### 🛡 Safety First
- **The Pug's Nose (Save Heuristics):** Before executing any deletion, PrefixPug sniffs through orphaned prefixes for game saves (`Saved Games/`, `Documents/`, `.sav`, `.ess`, etc.) and automatically buries (backs up) them to `~/.local/share/prefixpug/backups/`.
- **Zero Unsafe Defaults:** Always prompts for explicit user confirmation before touching files.
- **Robust Error Handling:** Pure `Result`-driven operations without panics.

---

## 🎨 Cyberpunk Synthwave TUI

PrefixPug features an interactive terminal UI built with `ratatui` with a Synthwave/Cyberpunk aesthetic (neon cyan & pink palette, grid-wire borders, animated pug sniffer, and interactive space-reclamation progress meter).

### Keybindings

| Key | Action |
| --- | --- |
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Space` | Toggle selection for current prefix |
| `a` | Select all / Deselect all |
| `c` | Clean selected prefixes (opens confirmation modal) |
| `q` / `Esc` | Quit PrefixPug |

---

## 🚀 CLI Mode

PrefixPug also runs headlessly in scripts or non-interactive environments:

```bash
# Dry run: scan and report orphaned prefixes without deleting
prefixpug --dry-run

# Run non-interactive CLI mode
prefixpug --no-tui

# Specify custom Steam library VDF path
prefixpug --library-vdf /path/to/libraryfolders.vdf

# Specify custom backup folder
prefixpug --backup-dir /path/to/backups
```

---

## 🏗 Architecture & Milestones

- **Phase 1 (Data Layer):** `vdf_parser.rs` & `scanner.rs` dynamically locate `libraryfolders.vdf`, parse installed games from `appmanifest_*.acf`, and detect orphaned AppIDs.
- **Phase 2 (CLI & Core Logic):** `cli.rs` (Clap definitions with `--dry-run` and `--auto-clean`) + `backup.rs` safe save archiving.
- **Phase 3 (TUI Dashboard):** `tui/app.rs` & `tui/ui.rs` interactive cyberpunk dashboard with live sniffing animations and space recovery gauge.
