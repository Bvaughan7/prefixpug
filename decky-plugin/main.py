import asyncio
import json
import os
import shutil
from pathlib import Path
import decky_plugin

class Plugin:
    def _find_prefixpug_binary(self) -> str:
        # Check bundled plugin binary first, then user path, then system path
        bundled = Path(decky_plugin.DECKY_PLUGIN_DIR) / "bin" / "prefixpug"
        if bundled.is_file() and os.access(bundled, os.X_OK):
            return str(bundled)

        home = Path(os.environ.get("HOME", "/home/deck"))
        user_local = home / ".local" / "bin" / "prefixpug"
        if user_local.is_file() and os.access(user_local, os.X_OK):
            return str(user_local)

        system_bin = shutil.which("prefixpug")
        if system_bin:
            return system_bin

        return "prefixpug"

    async def _run_command(self, args: list[str]) -> tuple[int, str, str]:
        binary = self._find_prefixpug_binary()
        cmd = [binary] + args
        decky_plugin.logger.info(f"PrefixPug executing: {' '.join(cmd)}")

        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )
        stdout, stderr = await proc.communicate()
        return (
            proc.returncode or 0,
            stdout.decode("utf-8", errors="replace"),
            stderr.decode("utf-8", errors="replace")
        )

    async def scan_orphans(self) -> dict:
        """Runs prefixpug scan --json and returns discovered orphaned prefixes."""
        code, stdout, stderr = await self._run_command(["scan", "--json"])
        if code in (0, 4):  # 0 = found orphans, 4 = 0 orphans found
            try:
                data = json.loads(stdout) if stdout.strip() else []
                return {"success": True, "orphans": data, "error": None}
            except Exception as e:
                return {"success": False, "orphans": [], "error": f"JSON parse error: {e}"}
        return {"success": False, "orphans": [], "error": stderr or f"Scan failed with code {code}"}

    async def audit_inventory(self, stale: bool = False) -> dict:
        """Runs prefixpug audit --json to inspect all installed, shortcut, and runtime prefixes."""
        args = ["audit", "--json"]
        if stale:
            args.append("--stale")

        code, stdout, stderr = await self._run_command(args)
        if code in (0, 4):
            try:
                data = json.loads(stdout) if stdout.strip() else []
                return {"success": True, "prefixes": data, "error": None}
            except Exception as e:
                return {"success": False, "prefixes": [], "error": f"JSON parse error: {e}"}
        return {"success": False, "prefixes": [], "error": stderr or f"Audit failed with code {code}"}

    async def clean_orphans(self, appids: list[str], shaders_only: bool = False) -> dict:
        """Safely cleans selected prefixes after auto-vaulting saves."""
        args = ["clean", "--yes"]
        if shaders_only:
            args.append("--shaders-only")
        if appids:
            args.extend(["--appids", ",".join(appids)])

        code, stdout, stderr = await self._run_command(args)
        if code == 0:
            return {"success": True, "output": stdout, "error": None}
        return {"success": False, "output": stdout, "error": stderr or f"Clean failed with code {code}"}

    async def vault_prefix(self, appid: str) -> dict:
        """Vaults save files for a specific AppID without prefix deletion."""
        code, stdout, stderr = await self._run_command(["vault", str(appid)])
        if code == 0:
            return {"success": True, "output": stdout, "error": None}
        return {"success": False, "output": stdout, "error": stderr or f"Vault failed with code {code}"}

    async def list_backups(self) -> dict:
        """Lists all archived save vaults."""
        code, stdout, stderr = await self._run_command(["backups", "--json"])
        if code in (0, 4):
            try:
                data = json.loads(stdout) if stdout.strip() else []
                return {"success": True, "backups": data, "error": None}
            except Exception as e:
                return {"success": False, "backups": [], "error": str(e)}
        return {"success": False, "backups": [], "error": stderr}

    async def _main(self):
        decky_plugin.logger.info("PrefixPug Decky plugin backend loaded successfully")

    async def _unload(self):
        decky_plugin.logger.info("PrefixPug Decky plugin backend unloaded")
