# Changelog

All notable changes to **PrefixPug** are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.0] - 2026-09-03

### Added
- **Multi-Library Mount Guard:** Validates that all storage drives referenced in `libraryfolders.vdf` are reachable and mounted. If any drive is missing or unmounted, PrefixPug halts immediately (Exit Code 2) to prevent misclassifying games on disconnected drives as orphans.
- **Non-Steam Shortcut Protection:** Added a binary VDF parser for `shortcuts.vdf` across all user profiles, computing 32-bit CRC appids and preserving Battle.net, EGS, emulator, and Heroic prefixes.
- **Runtime Infrastructure Deny-List:** Critical Steam and Proton runtimes (Steam Linux Runtime, Proton Experimental, Proton Hotfix, Steamworks Common Redistributables) are marked non-deletable and locked from cleanup.
- **The Pug's Nose (Blocklist Save Engine):** Saves are sniffed across entire user directory trees (`Saved Games`, `AppData`, `Documents`) preserving extensionless, `.json`, `.xml`, and `.dat` saves.
- **Save Vault Integrity & fsync:** Every save archive is compressed with `flate2`, audited with per-file and archive-level SHA-256 hashes in `manifest.json`, flushed with `fsync`, and validated via read decompression before any unlinking is allowed.
- **Standalone `vault` Subcommand:** Direct save extraction and vaulting with `prefixpug vault <APPID_OR_PATH>`.
- **Structured Exit Codes:**
  - `0`: Clean / Success
  - `1`: General Error
  - `2`: Unsafe State Detected (Steam running, unmounted drive, escaping symlink)
  - `3`: User Canceled / Aborted Confirmation
  - `4`: No matching orphaned prefixes found
- **Accurate Disk Accounting:** Measures allocated disk blocks (`st_blocks * 512`) rather than apparent file size to properly reflect sparse prefix files, and reports measured free space deltas via `statvfs`.
- **Low-Risk Shader Cleanup:** Dedicated `prefixpug shaders --yes` and `--shaders-only` commands.
- **Mod Loader & High-Value Flagging:** Detects SKSE, F4SE, NVSE, OBSE, MWSE, ModOrganizer, and protontricks logs, highlighting them in the TUI and CLI.
- **Man Page & Packaging:** Added `man/prefixpug.1` manual page and `packaging/PKGBUILD` for Arch/AUR.
- **Safety Documentation:** Added [`SAFETY.md`](SAFETY.md) outlining threat models, fail-safes, and technical limitations.

### Changed
- **Safe-by-Default CLI:** `prefixpug clean` now runs in `--dry-run` mode by default. Headless non-interactive deletion requires `--yes` or `--purge`, and interactive sessions prompt for confirmation.
- **TUI Density:** Mascot banner is collapsible/togglable via `m`, defaulting to a data-dense layout.

---

## [0.1.0] - 2026-09-03

### Added
- Initial project prototype.
- Steam library discovery and VDF parsing via `keyvalues-parser`.
- Ratatui terminal user interface with Cyberpunk neon theme.
- Basic save-game backup mechanism.
