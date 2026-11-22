import { ShieldAlert, ShieldCheck } from "lucide-react";
import { PanelCard } from "./UiPrimitives";
import type { CloakStatus } from "../types/crosshair";
import type { ThemeSurfaceMode } from "../types/themes";

interface ShieldTabProps {
  streamproof: boolean;
  cloakMenu: boolean;
  cloakStatus: CloakStatus | null;
  surfaceMode: ThemeSurfaceMode;
  onToggleCloak: (enabled: boolean, cloakMenu?: boolean) => void;
}

export function ShieldTab({
  streamproof,
  cloakMenu,
  cloakStatus,
  surfaceMode,
  onToggleCloak,
}: ShieldTabProps) {
  return (
    <div className="space-y-4">
      <PanelCard surfaceMode={surfaceMode} className="space-y-5 p-5">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="flex min-w-0 flex-1 items-start gap-3.5">
            <div
              className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-xl transition-colors ${
                streamproof
                  ? "bg-emerald-500/15 text-emerald-400 shadow-[0_0_24px_rgba(52,211,153,0.25)]"
                  : "bg-[var(--surface-subtle)] text-[var(--muted-text)]"
              }`}
            >
              {streamproof ? (
                <ShieldCheck className="h-6 w-6" />
              ) : (
                <ShieldAlert className="h-6 w-6" />
              )}
            </div>
            <div className="min-w-0">
              <h2 className="text-sm font-black tracking-wide">
                OBS & Discord Capture Cloak
              </h2>
              <p className="mt-1 text-[11px] leading-relaxed text-[var(--muted-text)]">
                Excludes the Luair crosshair overlay from OBS Game, Window, and Display
                capture, Discord screenshares, and OS screenshots while keeping it crisp
                on your monitor.
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={() => onToggleCloak(!streamproof, cloakMenu)}
            style={
              streamproof
                ? undefined
                : {
                    backgroundColor: "var(--accent)",
                    color: "var(--accent-fg)",
                  }
            }
            className={`shrink-0 rounded-xl px-4 py-2.5 text-xs font-bold transition ${
              streamproof
                ? "bg-emerald-400 text-slate-950 shadow-[0_0_24px_rgba(52,211,153,0.35)] hover:bg-emerald-300"
                : "hover:opacity-90"
            }`}
          >
            {streamproof ? "Shield Enabled" : "Enable Shield"}
          </button>
        </div>

        <div
          style={{ borderColor: "var(--border-subtle)" }}
          className="grid grid-cols-1 gap-3 border-t pt-4 sm:grid-cols-3"
        >
          <div
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
            }}
            className="rounded-xl border p-3"
          >
            <div className="text-[10px] font-semibold tracking-wider uppercase text-[var(--muted-text)]">
              Platform
            </div>
            <div className="mt-1 font-mono text-xs font-bold">
              {cloakStatus?.platform ?? "detecting..."}
            </div>
          </div>
          <div
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
            }}
            className="rounded-xl border p-3"
          >
            <div className="text-[10px] font-semibold tracking-wider uppercase text-[var(--muted-text)]">
              Native API
            </div>
            <div className="mt-1 font-mono text-[11px] font-bold text-[var(--accent)]">
              {cloakStatus?.mechanism ?? "WDA_EXCLUDEFROMCAPTURE"}
            </div>
          </div>
          <div
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
            }}
            className="rounded-xl border p-3"
          >
            <div className="text-[10px] font-semibold tracking-wider uppercase text-[var(--muted-text)]">
              Kernel Affinity Check
            </div>
            <div
              className={`mt-1 font-mono text-xs font-bold ${
                cloakStatus?.overlayVerified ? "text-emerald-400" : "text-amber-400"
              }`}
            >
              {cloakStatus?.overlayVerified
                ? streamproof
                  ? "VERIFIED CLOAKED"
                  : "VERIFIED VISIBLE"
                : "UNVERIFIED"}
            </div>
          </div>
        </div>

        <label
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="flex cursor-pointer items-center justify-between gap-4 rounded-xl border p-3.5"
        >
          <div>
            <div className="text-xs font-bold">Also Cloak Management Menu</div>
            <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              Hides this settings window from OBS and Discord capture as well when Shield
              is enabled.
            </div>
          </div>
          <input
            type="checkbox"
            checked={cloakMenu}
            onChange={(e) => onToggleCloak(streamproof, e.target.checked)}
          />
        </label>
      </PanelCard>
    </div>
  );
}
