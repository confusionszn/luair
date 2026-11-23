import {
  Database,
  DownloadCloud,
  Globe,
  Keyboard,
  LogOut,
  Monitor,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { PanelCard } from "./UiPrimitives";
import type { AuthSession } from "../types/auth";
import type { ThemeSurfaceMode } from "../types/themes";
import type { PlatformBuildInfo, UpdateCheckResult } from "../types/updater";

interface SettingsTabProps {
  menuHotkey: string;
  adsHotkey: string | null;
  recordingKind: "menu" | "ads" | null;
  session: AuthSession;
  surfaceMode: ThemeSurfaceMode;
  updateInfo: UpdateCheckResult | null;
  checkingUpdate: boolean;
  platformInfo: PlatformBuildInfo | null;
  onStartRecording: (kind: "menu" | "ads") => void;
  onResetOverlayBounds: () => void;
  onReplayOnboarding: () => void;
  onSignOut: () => void;
  onCheckUpdates: () => void;
  onOpenUpdateModal: () => void;
}

export function SettingsTab({
  menuHotkey,
  adsHotkey,
  recordingKind,
  session,
  surfaceMode,
  updateInfo,
  checkingUpdate,
  platformInfo,
  onStartRecording,
  onResetOverlayBounds,
  onReplayOnboarding,
  onSignOut,
  onCheckUpdates,
  onOpenUpdateModal,
}: SettingsTabProps) {
  const statusBadge = (() => {
    if (checkingUpdate) {
      return {
        label: "Checking GitHub...",
        cls: "border-sky-400/35 bg-sky-500/15 text-sky-300",
      };
    }
    if (updateInfo?.available) {
      return {
        label: `Update v${updateInfo.latestVersion}`,
        cls: "border-emerald-400/35 bg-emerald-500/15 text-emerald-300",
      };
    }
    if (updateInfo?.status === "offline-or-unreleased") {
      return {
        label: "Offline / Unreleased",
        cls: "border-amber-400/35 bg-amber-500/15 text-amber-300",
      };
    }
    return {
      label: "Up to Date",
      cls: "border-emerald-400/30 bg-emerald-500/15 text-emerald-400",
    };
  })();

  return (
    <div className="space-y-4">
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <h2 className="flex items-center gap-2 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
          <Keyboard className="h-3.5 w-3.5" />
          Global Hotkeys
        </h2>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="flex items-center justify-between gap-3 rounded-xl border p-3.5"
        >
          <div>
            <div className="text-xs font-bold">Menu Visibility Hotkey</div>
            <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              Supports single keys (F10, Home, Insert) or combos (Ctrl+Shift+X).
            </div>
          </div>
          <button
            type="button"
            onClick={() => onStartRecording("menu")}
            style={{
              backgroundColor:
                recordingKind === "menu" ? "var(--accent)" : "var(--panel-color)",
              borderColor:
                recordingKind === "menu" ? "var(--accent)" : "var(--border-subtle)",
              color: recordingKind === "menu" ? "var(--accent-fg)" : "var(--accent)",
            }}
            className={`min-w-[134px] rounded-lg border px-3 py-1.5 font-mono text-xs font-bold transition ${
              recordingKind === "menu" ? "animate-pulse" : "hover:border-[var(--accent)]"
            }`}
          >
            {recordingKind === "menu" ? "PRESS COMBO..." : menuHotkey}
          </button>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="flex items-center justify-between gap-3 rounded-xl border p-3.5"
        >
          <div>
            <div className="text-xs font-bold">ADS / Crosshair Toggle Hotkey</div>
            <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              Quickly hide or show the crosshair (Backspace/Delete while recording clears).
            </div>
          </div>
          <button
            type="button"
            onClick={() => onStartRecording("ads")}
            style={{
              backgroundColor:
                recordingKind === "ads" ? "var(--accent)" : "var(--panel-color)",
              borderColor:
                recordingKind === "ads" ? "var(--accent)" : "var(--border-subtle)",
              color: recordingKind === "ads" ? "var(--accent-fg)" : "var(--accent)",
            }}
            className={`min-w-[134px] rounded-lg border px-3 py-1.5 font-mono text-xs font-bold transition ${
              recordingKind === "ads" ? "animate-pulse" : "hover:border-[var(--accent)]"
            }`}
          >
            {recordingKind === "ads" ? "PRESS COMBO..." : (adsHotkey ?? "UNBOUND")}
          </button>
        </div>
      </PanelCard>

      {/* Software Updates (GitHub Releases) */}
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="flex items-center gap-2 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            <DownloadCloud className="h-3.5 w-3.5" />
            In-App Auto-Updater (GitHub Releases)
          </h2>
          <span
            className={`rounded-full border px-2 py-0.5 text-[10px] font-semibold ${statusBadge.cls}`}
          >
            {statusBadge.label}
          </span>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="space-y-2.5 rounded-xl border p-3.5"
        >
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <div className="text-xs font-bold">
                Current Version:{" "}
                <span className="font-mono text-[var(--accent)]">
                  v{platformInfo?.appVersion ?? updateInfo?.currentVersion ?? "2.0.2"}
                </span>
              </div>
              <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
                {updateInfo?.message ??
                  "Checks signed Tauri Updater manifests published on GitHub Releases."}
              </div>
            </div>

            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                disabled={checkingUpdate}
                onClick={onCheckUpdates}
                style={{
                  backgroundColor: "var(--surface-subtle)",
                  borderColor: "var(--border-subtle)",
                }}
                className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)] disabled:opacity-60"
              >
                <RefreshCw
                  className={`h-3.5 w-3.5 text-[var(--accent)] ${
                    checkingUpdate ? "animate-spin" : ""
                  }`}
                />
                {checkingUpdate ? "Checking..." : "Check for Updates"}
              </button>

              {updateInfo?.available && (
                <button
                  type="button"
                  onClick={onOpenUpdateModal}
                  className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-[11px] font-bold text-[var(--accent-fg)] shadow-[0_0_14px_var(--accent-glow)] transition hover:opacity-90"
                >
                  <DownloadCloud className="h-3.5 w-3.5" />
                  View Release Notes & Update Now
                </button>
              )}
            </div>
          </div>

          <div className="truncate font-mono text-[10px] opacity-60">
            Feed: {updateInfo?.endpoint ?? platformInfo?.updaterEndpoint ?? "GitHub Releases latest.json"}
          </div>
        </div>
      </PanelCard>

      {/* Local Account Vault Card */}
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="flex items-center gap-2 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            <Database className="h-3.5 w-3.5" />
            Local Account Vault
          </h2>
          <span className="rounded-full border border-emerald-400/30 bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-400">
            {session.rememberMe ? "Session Remembered" : "Session Active"}
          </span>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="flex flex-wrap items-center justify-between gap-3 rounded-xl border p-3.5"
        >
          <div>
            <div className="text-xs font-bold">
              @{session.account?.username ?? "local"}
            </div>
            <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              {session.account?.email} · {session.vaultEngine}
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={onReplayOnboarding}
              style={{
                backgroundColor: "var(--surface-subtle)",
                borderColor: "var(--border-subtle)",
              }}
              className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
            >
              <Sparkles className="h-3.5 w-3.5 text-[var(--accent)]" />
              Replay Setup Wizard
            </button>
            <button
              type="button"
              onClick={onSignOut}
              className="flex items-center gap-1.5 rounded-lg border border-rose-500/25 bg-rose-500/10 px-3 py-1.5 text-[11px] font-bold text-rose-300 transition hover:bg-rose-500/20"
            >
              <LogOut className="h-3.5 w-3.5" />
              Sign Out
            </button>
          </div>
        </div>
      </PanelCard>

      <PanelCard
        surfaceMode={surfaceMode}
        className="flex flex-wrap items-center justify-between gap-3 p-4"
      >
        <div>
          <div className="text-xs font-bold">Overlay Monitor Alignment</div>
          <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
            Re-centers and fits the transparent overlay window to the primary monitor
            bounds.
          </div>
        </div>
        <button
          type="button"
          onClick={onResetOverlayBounds}
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="flex items-center gap-1.5 rounded-lg border px-3.5 py-2 text-xs font-semibold transition hover:border-[var(--accent)]"
        >
          <Monitor className="h-3.5 w-3.5 text-[var(--accent)]" />
          Reset Overlay Position
        </button>
      </PanelCard>

      {/* Cross-Platform Runtime & Targets */}
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="flex items-center gap-2 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            <Globe className="h-3.5 w-3.5" />
            Cross-Platform Runtime & Build Targets
          </h2>
          <span className="rounded-full border border-[var(--border-subtle)] px-2.5 py-0.5 font-mono text-[10px] font-bold text-[var(--accent)]">
            {platformInfo?.targetTriple ?? "Tauri v2 + Rust"}
          </span>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="grid gap-2 rounded-xl border p-3.5 text-[11px] sm:grid-cols-2"
        >
          <div>
            <span className="text-[var(--muted-text)]">Display / Composition:</span>
            <div className="mt-0.5 font-semibold">
              {platformInfo?.displayServer ?? "DirectX / GDI / Wayland / Quartz"}
            </div>
          </div>
          <div>
            <span className="text-[var(--muted-text)]">Overlay Backend:</span>
            <div className="mt-0.5 font-semibold">
              {platformInfo?.overlayBackend ?? "Independent Webview Overlay"}
            </div>
          </div>
        </div>

        <div className="flex flex-wrap gap-1.5">
          {(
            platformInfo?.packagingTargets ?? [
              "Windows MSVC x86_64 (NSIS & MSI)",
              "Linux Debian (.deb)",
              "Linux Arch (PKGBUILD)",
              "Linux Fedora (.rpm)",
              "Linux Flatpak & AppImage (X11/Wayland)",
              "macOS Universal Binary (x86_64 + arm64 DMG)",
            ]
          ).map((target) => (
            <span
              key={target}
              style={{
                backgroundColor: "var(--surface-subtle)",
                borderColor: "var(--border-subtle)",
              }}
              className="rounded-lg border px-2 py-1 font-mono text-[10px] opacity-85"
            >
              {target}
            </span>
          ))}
        </div>
      </PanelCard>
    </div>
  );
}
