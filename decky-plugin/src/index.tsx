import {
  ButtonItem,
  definePlugin,
  DialogButton,
  Field,
  Navigation,
  PanelSection,
  PanelSectionRow,
  ServerAPI,
  showModal,
  ConfirmModal,
  staticClasses,
} from "@decky/ui";
import { FC, useState, useEffect } from "react";
import { FaTrash, FaShieldAlt, FaSync, FaArchive, FaGamepad } from "react-icons/fa";

interface OrphanItem {
  appid: string;
  title: string | null;
  compatdata_usage: { apparent_bytes: number; allocated_bytes: number };
  shadercache_usage: { apparent_bytes: number; allocated_bytes: number };
  detected_saves: Array<{ path: string; size_bytes: number }>;
  is_high_value: boolean;
  cloud_status: string;
}

function formatBytes(bytes: number): string {
  const KIB = 1024;
  const MIB = KIB * 1024;
  const GIB = MIB * 1024;
  if (bytes >= GIB) return (bytes / GIB).toFixed(2) + " GiB";
  if (bytes >= MIB) return (bytes / MIB).toFixed(1) + " MiB";
  if (bytes >= KIB) return (bytes / KIB).toFixed(0) + " KiB";
  return bytes + " B";
}

const PrefixPugContent: FC<{ serverApi: ServerAPI }> = ({ serverApi }) => {
  const [loading, setLoading] = useState(false);
  const [statusMsg, setStatusMsg] = useState("Sniffing storage...");
  const [orphans, setOrphans] = useState<OrphanItem[]>([]);
  const [reclaimable, setReclaimable] = useState(0);

  const fetchOrphans = async () => {
    setLoading(true);
    setStatusMsg("Scanning for orphaned compatdata...");
    try {
      const res = await serverApi.callPluginMethod<{}, { success: boolean; orphans: OrphanItem[]; error?: string }>(
        "scan_orphans",
        {}
      );
      if (res.success && res.result?.success) {
        const list = res.result.orphans || [];
        setOrphans(list);
        let total = 0;
        for (const item of list) {
          total += (item.compatdata_usage?.apparent_bytes || 0) + (item.shadercache_usage?.apparent_bytes || 0);
        }
        setReclaimable(total);
        setStatusMsg(list.length > 0 ? `Found ${list.length} orphaned prefixes` : "Storage is clean! No orphans found.");
      } else {
        setStatusMsg(`Scan error: ${res.result?.error || "Unknown"}`);
      }
    } catch (err: any) {
      setStatusMsg(`Connection error: ${err?.message || err}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchOrphans();
  }, []);

  const handleCleanAll = (shadersOnly = false) => {
    showModal(
      <ConfirmModal
        onOK={async () => {
          setLoading(true);
          setStatusMsg(shadersOnly ? "Cleaning shader caches..." : "Vaulting saves & cleaning prefixes...");
          try {
            const res = await serverApi.callPluginMethod<{ appids: string[]; shaders_only: boolean }, any>(
              "clean_orphans",
              { appids: [], shaders_only: shadersOnly }
            );
            if (res.success && res.result?.success) {
              setStatusMsg(shadersOnly ? "Shader caches cleaned!" : "Prefixes cleaned and saves safely vaulted!");
              fetchOrphans();
            } else {
              setStatusMsg(`Cleanup failed: ${res.result?.error || "Unknown"}`);
            }
          } catch (e: any) {
            setStatusMsg(`Error: ${e?.message || e}`);
          } finally {
            setLoading(false);
          }
        }}
      >
        <p>
          {shadersOnly
            ? "Clean all orphaned GPU shader caches? This is zero-risk and preserves all Proton prefixes."
            : `Are you sure you want to clean ${orphans.length} orphaned prefix(es) and reclaim ${formatBytes(reclaimable)}?`}
        </p>
        {!shadersOnly && (
          <p style={{ color: "#00ffff", fontSize: "12px", marginTop: "8px" }}>
            🛡 The Pug's Nose will automatically vault all local save files to ~/.local/share/prefixpug/backups/ before deletion!
          </p>
        )}
      </ConfirmModal>
    );
  };

  return (
    <PanelSection title="PrefixPug Storage Sniffer">
      <PanelSectionRow>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ color: "#ff007f", fontWeight: "bold" }}>⚡ CYBERPUG DISK SENTINEL</div>
            <div style={{ fontSize: "12px", color: "#888" }}>{statusMsg}</div>
          </div>
          <DialogButton onClick={fetchOrphans} disabled={loading} style={{ minWidth: "40px" }}>
            <FaSync />
          </DialogButton>
        </div>
      </PanelSectionRow>

      {orphans.length > 0 && (
        <>
          <PanelSectionRow>
            <Field label="Reclaimable Space" description={`${orphans.length} orphaned prefix(es)`}>
              <span style={{ color: "#00ffcc", fontWeight: "bold", fontSize: "16px" }}>
                {formatBytes(reclaimable)}
              </span>
            </Field>
          </PanelSectionRow>

          <PanelSectionRow>
            <ButtonItem
              layout="below"
              onClick={() => handleCleanAll(false)}
              disabled={loading}
              style={{ backgroundColor: "#ff007f" }}
            >
              <FaShieldAlt style={{ marginRight: "6px" }} />
              Vault Saves & Clean All ({formatBytes(reclaimable)})
            </ButtonItem>
          </PanelSectionRow>

          <PanelSectionRow>
            <ButtonItem
              layout="below"
              onClick={() => handleCleanAll(true)}
              disabled={loading}
            >
              <FaTrash style={{ marginRight: "6px" }} />
              Clean Shader Caches Only (Zero Risk)
            </ButtonItem>
          </PanelSectionRow>
        </>
      )}

      {orphans.slice(0, 8).map((orphan) => {
        const itemSize = (orphan.compatdata_usage?.apparent_bytes || 0) + (orphan.shadercache_usage?.apparent_bytes || 0);
        return (
          <PanelSectionRow key={orphan.appid}>
            <Field
              label={orphan.title || `AppID ${orphan.appid}`}
              description={`AppID: ${orphan.appid} • Saves: ${orphan.detected_saves?.length || 0}`}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <span style={{ color: "#00ffff" }}>{formatBytes(itemSize)}</span>
                <DialogButton
                  onClick={async () => {
                    await serverApi.callPluginMethod("vault_prefix", { appid: orphan.appid });
                    setStatusMsg(`Vaulted saves for ${orphan.appid}!`);
                  }}
                  style={{ minWidth: "30px", padding: "4px 8px" }}
                >
                  <FaArchive title="Vault Saves" />
                </DialogButton>
              </div>
            </Field>
          </PanelSectionRow>
        );
      })}
    </PanelSection>
  );
};

export default definePlugin((serverApi: ServerAPI) => {
  return {
    title: <div className={staticClasses.Title}>PrefixPug</div>,
    content: <PrefixPugContent serverApi={serverApi} />,
    icon: <FaGamepad />,
  };
});
