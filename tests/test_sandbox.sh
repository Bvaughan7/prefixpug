#!/usr/bin/env bash
set -euo pipefail

# PrefixPug Mock Steam Sandbox Test & Demo
SANDBOX="/tmp/prefixpug_mock_steam"
rm -rf "$SANDBOX"
mkdir -p "$SANDBOX/steamapps/compatdata"
mkdir -p "$SANDBOX/steamapps/shadercache"

echo -e "\033[1;35m⚡ Setting up Mock Steam Sandbox at ${SANDBOX}...\033[0m"

# 1. Create an INSTALLED game (AppID 100)
cat << 'APP' > "$SANDBOX/steamapps/appmanifest_100.acf"
"AppState"
{
	"appid"		"100"
	"name"		"Cyberpunk 2077"
	"installdir"	"Cyberpunk2077"
	"SizeOnDisk"	"70000000000"
}
APP
mkdir -p "$SANDBOX/steamapps/compatdata/100/pfx/drive_c/users/steamuser"
echo "Active player profile" > "$SANDBOX/steamapps/compatdata/100/pfx/drive_c/users/steamuser/active.cfg"

# 2. Create an ORPHANED prefix (AppID 489830 - Skyrim SE)
SKYRIM_PFX="$SANDBOX/steamapps/compatdata/489830/pfx"
mkdir -p "$SKYRIM_PFX/drive_c/users/steamuser/Saved Games/Skyrim Special Edition"
mkdir -p "$SKYRIM_PFX/drive_c/users/steamuser/Documents/My Games/Skyrim Special Edition"
mkdir -p "$SANDBOX/steamapps/shadercache/489830"

cat << 'REG' > "$SKYRIM_PFX/user.reg"
[Software\\Bethesda\\Skyrim Special Edition]
"Installed"=dword:00000001
REG

echo "SKYRIM LEVEL 85 DRAGONBORN SAVE DATA" > "$SKYRIM_PFX/drive_c/users/steamuser/Saved Games/Skyrim Special Edition/Save1.ess"
echo "AUTOSAVE SKYRIM DUNGEON" > "$SKYRIM_PFX/drive_c/users/steamuser/Documents/My Games/Skyrim Special Edition/quicksave.sav"
dd if=/dev/zero of="$SKYRIM_PFX/drive_c/system_files.bin" bs=1M count=100 2>/dev/null
dd if=/dev/zero of="$SANDBOX/steamapps/shadercache/489830/shaders.bin" bs=1M count=25 2>/dev/null

# 3. Create another ORPHANED prefix (AppID 1091500 - CyberPug)
PUG_PFX="$SANDBOX/steamapps/compatdata/1091500/pfx"
mkdir -p "$PUG_PFX/drive_c/users/steamuser/AppData/Local/CyberPug"
mkdir -p "$SANDBOX/steamapps/shadercache/1091500"

cat << 'REG' > "$PUG_PFX/user.reg"
[Software\\NeonForge\\CyberPug Chronicles]
"Version"="1.0"
REG

echo "CYBERPUG ULTRA HIGH SCORE: 999999" > "$PUG_PFX/drive_c/users/steamuser/AppData/Local/CyberPug/profile.dat"
dd if=/dev/zero of="$PUG_PFX/drive_c/data.bin" bs=1M count=50 2>/dev/null

# 4. Generate libraryfolders.vdf
cat << VDF > "$SANDBOX/steamapps/libraryfolders.vdf"
"libraryfolders"
{
	"0"
	{
		"path"		"$SANDBOX"
		"label"		"NVMe Game Drive"
		"apps"
		{
			"100"		"70000000000"
		}
	}
}
VDF

echo -e "\033[1;32m✓ Sandbox ready!\033[0m"
echo ""
echo "Try running:"
echo "  1) prefixpug --library-vdf $SANDBOX/steamapps/libraryfolders.vdf scan"
echo "  2) prefixpug --library-vdf $SANDBOX/steamapps/libraryfolders.vdf (Launches Cyberpunk TUI on the sandbox!)"
echo "  3) prefixpug --library-vdf $SANDBOX/steamapps/libraryfolders.vdf clean --dry-run"
echo ""
