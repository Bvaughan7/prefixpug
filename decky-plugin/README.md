# PrefixPug Decky Loader Plugin 🐾

This directory contains the **Decky Loader plugin** frontend and backend for PrefixPug on the Steam Deck and SteamOS.

---

## 🎮 Features on Steam Deck

- **Quick Access Menu (QAM) Integration:** Access PrefixPug directly by pressing the `...` button in gaming mode.
- **One-Tap Safe Reclamation:** Scans internal SSD and SD card (`/run/media/mmcblk0p1`) and calculates total reclaimable space.
- **The Pug's Nose Safety:** Automatically vaults local saves to `~/.local/share/prefixpug/backups/` before deleting any prefix.
- **Zero-Risk Shader Cache Purge:** Clean GPU shader caches without touching any Wine/Proton prefixes.
- **Save Vaulting per Title:** Extract and backup saves for any title on demand.

---

## 🛠️ Architecture

The Decky plugin uses PrefixPug's safe, headless `--json` interface:

```
┌────────────────────────────────────────────────┐
│           SteamOS Quick Access Menu            │
│       React UI (@decky/ui / index.tsx)         │
└───────────────────────┬────────────────────────┘
                        │ IPC
┌───────────────────────▼────────────────────────┐
│            Decky Plugin Python Server          │
│                    (main.py)                   │
└───────────────────────┬────────────────────────┘
                        │ Subprocess
┌───────────────────────▼────────────────────────┐
│              prefixpug binary                  │
│       (Rust core with safe dry-run defaults)   │
└────────────────────────────────────────────────┘
```

---

## 📦 Installation on Steam Deck

1. Install [Decky Loader](https://decky.xyz/) if not already installed.
2. Ensure PrefixPug is installed on your Deck:
   ```bash
   curl -sSL https://raw.githubusercontent.com/PrefixPug/prefixpug/main/install.sh | bash
   ```
3. Copy this plugin directory to `~/homebrew/plugins/prefixpug`:
   ```bash
   mkdir -p ~/homebrew/plugins/prefixpug
   cp -r * ~/homebrew/plugins/prefixpug/
   ```
4. Restart the Decky plugin loader or reboot into Gaming Mode.
