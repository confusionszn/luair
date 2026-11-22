import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, MotionConfig } from "framer-motion";
import {
  Bot,
  Copy,
  Crosshair,
  DownloadCloud,
  EyeOff,
  Layers,
  LogOut,
  Maximize2,
  Minus,
  Palette,
  Power,
  Settings,
  Shield,
  ShieldCheck,
} from "lucide-react";
import { AiAssistantTab } from "../components/AiAssistantTab";
import { CrosshairTab } from "../components/CrosshairTab";
import { DynamicGlassShell } from "../components/DynamicGlassShell";
import { LuairLogo } from "../components/LuairLogo";
import { OnboardingFlow } from "../components/OnboardingFlow";
import { ProfilesTab } from "../components/ProfilesTab";
import { SettingsTab } from "../components/SettingsTab";
import { ShieldTab } from "../components/ShieldTab";
import { ThemeTab } from "../components/ThemeTab";
import { UpdateBanner } from "../components/UpdateBanner";
import { WindowResizeHandles } from "../components/WindowResizeHandles";
import {
  checkForUpdates,
  downloadAndInstallUpdate,
  getAuthSession,
  getCaptureCloakStatus,
  getPlatformInfo,
  getState,
  hideMenu,
  isMenuMaximized,
  minimizeMenu,
  onAuthUpdated,
  onMouseCaptured,
  onProfileSwitched,
  onStateUpdated,
  onTriggerUpdateCheck,
  onUpdateProgress,
  quitApp,
  reopenOnboarding,
  resetOverlayBounds,
  selectProfile,
  setCaptureCloak,
  setHotkeyRecording,
  signOutLocal,
  toggleMaximizeMenu,
  updateState,
} from "../lib/tauri";
import type {
  PlatformBuildInfo,
  UpdateCheckResult,
  UpdateProgressEvent,
} from "../types/updater";
import { DEFAULT_AUTH_SESSION, type AuthSession } from "../types/auth";
import {
  DEFAULT_CROSSHAIR_CONFIG,
  DEFAULT_LUAIR_STATE,
  type CloakStatus,
  type CrosshairConfig,
  type CrosshairProfile,
  type LuairState,
} from "../types/crosshair";
import { applyThemeToDocument, type ThemeConfig } from "../types/themes";

type MenuTab = "crosshair" | "profiles" | "shield" | "theme" | "ai" | "settings";

type RecordingTarget =
  | { kind: "menu" }
  | { kind: "ads" }
  | { kind: "profile"; profileId: string }
  | null;

function formatKeyFromEvent(e: KeyboardEvent): string | null {
  const modifierCodes = new Set([
    "ControlLeft",
    "ControlRight",
    "ShiftLeft",
    "ShiftRight",
    "AltLeft",
    "AltRight",
    "MetaLeft",
    "MetaRight",
  ]);
  if (modifierCodes.has(e.code)) {
    return null;
  }

  let keyToken: string | null = null;
  if (/^Digit[0-9]$/.test(e.code)) {
    keyToken = e.code.slice(5);
  } else if (/^Numpad[0-9]$/.test(e.code)) {
    keyToken = e.code;
  } else if (/^Key[A-Z]$/.test(e.code)) {
    keyToken = e.code.slice(3);
  } else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) {
    keyToken = e.code;
  } else {
    const codeMap: Record<string, string> = {
      Space: "Space",
      Backquote: "`",
      Minus: "-",
      Equal: "=",
      BracketLeft: "[",
      BracketRight: "]",
      Semicolon: ";",
      Quote: "'",
      Backslash: "\\",
      Comma: ",",
      Period: ".",
      Slash: "/",
      Home: "Home",
      End: "End",
      Insert: "Insert",
      Delete: "Delete",
      PageUp: "PageUp",
      PageDown: "PageDown",
      Tab: "Tab",
      CapsLock: "CapsLock",
      ArrowUp: "ArrowUp",
      ArrowDown: "ArrowDown",
      ArrowLeft: "ArrowLeft",
      ArrowRight: "ArrowRight",
      Enter: "Enter",
      Backspace: "Backspace",
    };
    keyToken = codeMap[e.code] ?? null;
  }

  if (!keyToken) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Meta");
  parts.push(keyToken);

  return parts.join("+");
}

export function MenuWindow() {
  const [state, setState] = useState<LuairState>(DEFAULT_LUAIR_STATE);
  const [loaded, setLoaded] = useState(false);
  const [session, setSession] = useState<AuthSession>(DEFAULT_AUTH_SESSION);
  const [authLoaded, setAuthLoaded] = useState(false);
  const [tab, setTab] = useState<MenuTab>("crosshair");
  const [cloakStatus, setCloakStatus] = useState<CloakStatus | null>(null);
  const [recording, setRecording] = useState<RecordingTarget>(null);
  const [switchedBanner, setSwitchedBanner] = useState<string | null>(null);
  const [maximized, setMaximized] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [updatePromptOpen, setUpdatePromptOpen] = useState(false);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgressEvent | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [platformInfo, setPlatformInfo] = useState<PlatformBuildInfo | null>(null);
  const pendingLocalCommitRef = useRef(false);
  const saveTimerRef = useRef<number | null>(null);

  const runUpdateCheck = async (openModalIfAvailable: boolean) => {
    setCheckingUpdate(true);
    setUpdateError(null);
    try {
      const result = await checkForUpdates();
      setUpdateInfo(result);
      if (result.available && openModalIfAvailable) {
        setUpdatePromptOpen(true);
      }
    } catch {
      // Keep UI responsive offline.
    } finally {
      setCheckingUpdate(false);
    }
  };

  useEffect(() => {
    let active = true;
    let unlistenState: (() => void) | undefined;
    let unlistenAuth: (() => void) | undefined;
    let unlistenProfile: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    let unlistenTriggerCheck: (() => void) | undefined;

    void getAuthSession()
      .then((s) => {
        if (active) {
          setSession(s);
          setAuthLoaded(true);
        }
      })
      .catch(() => {
        if (active) setAuthLoaded(true);
      });

    void getState()
      .then((initial) => {
        if (active) {
          setState(initial);
          setLoaded(true);
        }
      })
      .catch(() => {
        if (active) setLoaded(true);
      });

    void getCaptureCloakStatus()
      .then((status) => {
        if (active) setCloakStatus(status);
      })
      .catch(() => {});

    void isMenuMaximized()
      .then((val) => {
        if (active) setMaximized(val);
      })
      .catch(() => {});

    void getPlatformInfo()
      .then((info) => {
        if (active) setPlatformInfo(info);
      })
      .catch(() => {});

    void onStateUpdated((next) => {
      if (!active) return;
      if (pendingLocalCommitRef.current) {
        pendingLocalCommitRef.current = false;
        return;
      }
      setState(next);
    }).then((fn) => {
      unlistenState = fn;
    });

    void onAuthUpdated((nextSession) => {
      if (active) setSession(nextSession);
    }).then((fn) => {
      unlistenAuth = fn;
    });

    void onProfileSwitched((profileId) => {
      if (!active) return;
      setSwitchedBanner(profileId);
      window.setTimeout(() => {
        setSwitchedBanner((cur) => (cur === profileId ? null : cur));
      }, 1500);
    }).then((fn) => {
      unlistenProfile = fn;
    });

    void onUpdateProgress((prog) => {
      if (active) setUpdateProgress(prog);
    }).then((fn) => {
      unlistenProgress = fn;
    });

    void onTriggerUpdateCheck(() => {
      if (!active) return;
      setTab("settings");
      void runUpdateCheck(true);
    }).then((fn) => {
      unlistenTriggerCheck = fn;
    });

    const startupTimer = window.setTimeout(() => {
      if (active) void runUpdateCheck(true);
    }, 1500);

    return () => {
      active = false;
      window.clearTimeout(startupTimer);
      unlistenState?.();
      unlistenAuth?.();
      unlistenProfile?.();
      unlistenProgress?.();
      unlistenTriggerCheck?.();
    };
  }, []);

  useEffect(() => {
    applyThemeToDocument(state.ui.theme);
  }, [state.ui.theme]);

  const commitState = (updater: (prev: LuairState) => LuairState, immediate = false) => {
    setState((prev) => {
      const next = updater(prev);
      if (loaded) {
        if (saveTimerRef.current !== null) {
          window.clearTimeout(saveTimerRef.current);
          saveTimerRef.current = null;
        }
        const dispatch = () => {
          pendingLocalCommitRef.current = true;
          void updateState(next);
        };
        if (immediate) {
          dispatch();
        } else {
          saveTimerRef.current = window.setTimeout(dispatch, 110);
        }
      }
      return next;
    });
  };

  const commitRef = useRef(commitState);
  commitRef.current = commitState;

  const applyBindingToTarget = (target: RecordingTarget, binding: string | null) => {
    if (!target) return;
    if (target.kind === "menu") {
      if (!binding) return;
      commitRef.current(
        (prev) => ({ ...prev, ui: { ...prev.ui, hotkey: binding } }),
        true,
      );
    } else if (target.kind === "ads") {
      commitRef.current(
        (prev) => ({ ...prev, ui: { ...prev.ui, adsHotkey: binding } }),
        true,
      );
    } else if (target.kind === "profile") {
      const targetId = target.profileId;
      commitRef.current(
        (prev) => ({
          ...prev,
          profiles: prev.profiles.map((p) =>
            p.id === targetId ? { ...p, hotkey: binding } : p,
          ),
        }),
        true,
      );
    }
  };

  useEffect(() => {
    if (!recording) {
      void setHotkeyRecording(false);
      return;
    }

    void setHotkeyRecording(true);
    let active = true;
    let unlistenMouse: (() => void) | undefined;

    void onMouseCaptured((binding) => {
      if (!active) return;
      applyBindingToTarget(recording, binding);
      setRecording(null);
    }).then((fn) => {
      unlistenMouse = fn;
    });

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.code === "Escape") {
        setRecording(null);
        return;
      }

      if ((e.code === "Backspace" || e.code === "Delete") && recording.kind !== "menu") {
        applyBindingToTarget(recording, null);
        setRecording(null);
        return;
      }

      const formatted = formatKeyFromEvent(e);
      if (!formatted) return;

      applyBindingToTarget(recording, formatted);
      setRecording(null);
    };

    const handleMouseSuppress = (e: MouseEvent) => {
      if (e.button >= 1) {
        e.preventDefault();
        e.stopPropagation();
      }
    };

    const handleBlur = () => setRecording(null);
    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("mousedown", handleMouseSuppress, true);
    window.addEventListener("auxclick", handleMouseSuppress, true);
    window.addEventListener("contextmenu", handleMouseSuppress, true);
    window.addEventListener("blur", handleBlur);
    return () => {
      active = false;
      unlistenMouse?.();
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("mousedown", handleMouseSuppress, true);
      window.removeEventListener("auxclick", handleMouseSuppress, true);
      window.removeEventListener("contextmenu", handleMouseSuppress, true);
      window.removeEventListener("blur", handleBlur);
      void setHotkeyRecording(false);
    };
  }, [recording]);

  const updateCrosshair = <K extends keyof CrosshairConfig>(
    key: K,
    value: CrosshairConfig[K],
  ) => {
    commitState((prev) => {
      const nextCrosshair: CrosshairConfig = {
        ...prev.crosshair,
        [key]: value,
      };
      if (key === "colorMode") {
        nextCrosshair.rainbow = value === "rainbow";
      } else if (key === "rainbow") {
        nextCrosshair.colorMode = value ? "rainbow" : "solid";
      }
      return {
        ...prev,
        crosshair: nextCrosshair,
      };
    });
  };

  const handleToggleCloak = async (enabled: boolean, cloakMenu?: boolean) => {
    try {
      const status = await setCaptureCloak(enabled, cloakMenu);
      setCloakStatus(status);
    } catch {
      const refreshed = await getCaptureCloakStatus().catch(() => null);
      if (refreshed) setCloakStatus(refreshed);
    }
  };

  const handleThemeChange = (nextTheme: ThemeConfig, immediate = false) => {
    commitState(
      (prev) => ({
        ...prev,
        ui: {
          ...prev.ui,
          uiTheme: nextTheme.name,
          uiAccent: nextTheme.accent,
          theme: nextTheme,
        },
      }),
      immediate,
    );
  };

  const handleAddProfile = (rawName: string) => {
    if (state.profiles.length >= 10) return;
    const index = state.profiles.length + 1;
    const name = rawName.trim() || `Profile ${index}`;
    const newProfile: CrosshairProfile = {
      id: `slot-${Date.now()}`,
      name,
      hotkey: index <= 9 ? String(index) : null,
      config: { ...state.crosshair },
    };
    commitState(
      (prev) => ({
        ...prev,
        profiles: [...prev.profiles, newProfile],
        activeProfileId: newProfile.id,
      }),
      true,
    );
  };

  const handleSaveOverProfile = (id: string) => {
    commitState(
      (prev) => ({
        ...prev,
        profiles: prev.profiles.map((p) =>
          p.id === id ? { ...p, config: { ...prev.crosshair } } : p,
        ),
        activeProfileId: id,
      }),
      true,
    );
  };

  const handleRenameProfile = (id: string, name: string) => {
    commitState((prev) => ({
      ...prev,
      profiles: prev.profiles.map((p) => (p.id === id ? { ...p, name } : p)),
    }));
  };

  const handleDeleteProfile = (id: string) => {
    commitState(
      (prev) => {
        const remaining = prev.profiles.filter((p) => p.id !== id);
        return {
          ...prev,
          profiles: remaining,
          activeProfileId:
            prev.activeProfileId === id
              ? (remaining[0]?.id ?? null)
              : prev.activeProfileId,
        };
      },
      true,
    );
  };

  const handleManualBind = (profileId: string, hotkey: string | null) => {
    commitState(
      (prev) => ({
        ...prev,
        profiles: prev.profiles.map((p) =>
          p.id === profileId ? { ...p, hotkey } : p,
        ),
      }),
      true,
    );
    setRecording(null);
  };

  const handleImportCrosshair = (config: CrosshairConfig) => {
    commitState(
      (prev) => ({
        ...prev,
        crosshair: { ...config },
      }),
      true,
    );
  };

  const handleImportAsProfile = (config: CrosshairConfig, name: string) => {
    if (state.profiles.length >= 10) return;
    const index = state.profiles.length + 1;
    const newProfile: CrosshairProfile = {
      id: `slot-${Date.now()}`,
      name,
      hotkey: index <= 9 ? String(index) : null,
      config: { ...config },
    };
    commitState(
      (prev) => ({
        ...prev,
        crosshair: { ...config },
        profiles: [...prev.profiles, newProfile],
        activeProfileId: newProfile.id,
      }),
      true,
    );
  };

  const handleToggleMaximize = async () => {
    const next = await toggleMaximizeMenu();
    setMaximized(next);
  };

  const handleSignOut = async () => {
    const next = await signOutLocal();
    setSession(next);
  };

  const handleReplayOnboarding = async () => {
    const next = await reopenOnboarding();
    setSession(next);
  };

  const handleInstallUpdate = async () => {
    setInstallingUpdate(true);
    setUpdateError(null);
    setUpdateProgress({
      stage: "downloading",
      downloadedBytes: 0,
      totalBytes: null,
      percent: 0,
      message: "Connecting to GitHub Releases...",
    });
    try {
      await downloadAndInstallUpdate();
    } catch (err) {
      setUpdateError(
        err instanceof Error ? err.message : "Failed to download and install update.",
      );
      setInstallingUpdate(false);
    }
  };

  const { crosshair, ui, profiles, activeProfileId } = state;
  const activeProfile = profiles.find((p) => p.id === activeProfileId);
  const theme = ui.theme;
  const isReadyInStudio =
    authLoaded && session.authenticated && Boolean(session.account?.onboardingCompleted);

  const navItems: { id: MenuTab; label: string; icon: typeof Crosshair }[] = [
    { id: "crosshair", label: "Crosshair", icon: Crosshair },
    { id: "profiles", label: "Profiles", icon: Layers },
    { id: "shield", label: "OBS Shield", icon: Shield },
    { id: "theme", label: "Theme Studio", icon: Palette },
    { id: "ai", label: "Luair AI", icon: Bot },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  return (
    <MotionConfig reducedMotion="user">
      <div className="h-screen w-screen overflow-hidden bg-transparent">
        <WindowResizeHandles disabled={maximized} />

        <DynamicGlassShell theme={theme} isMaximized={maximized}>
          {/* Frameless Draggable Titlebar */}
          <header
            style={{ borderColor: "var(--border-subtle)" }}
            className="flex h-12 shrink-0 items-center justify-between border-b px-4"
          >
            <div
              data-tauri-drag-region
              onDoubleClick={() => void handleToggleMaximize()}
              className="flex min-w-0 flex-1 cursor-move items-center gap-3 py-2"
            >
              <LuairLogo size={28} showWordmark />

              {isReadyInStudio && (
                <div className="hidden items-center gap-2 sm:flex">
                  {activeProfile && (
                    <span
                      style={{
                        backgroundColor: "rgba(var(--accent-rgb), 0.12)",
                        borderColor: "rgba(var(--accent-rgb), 0.35)",
                      }}
                      className="truncate rounded-full border px-2.5 py-0.5 text-[10px] font-semibold text-[var(--accent)]"
                    >
                      {activeProfile.name}
                      {activeProfile.hotkey ? ` [${activeProfile.hotkey}]` : ""}
                    </span>
                  )}
                  {cloakStatus?.enabled && (
                    <span className="flex items-center gap-1 rounded-full border border-emerald-400/30 bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-400">
                      <ShieldCheck className="h-3 w-3" />
                      Cloaked
                    </span>
                  )}
                  {updateInfo?.available && (
                    <button
                      type="button"
                      onClick={() => setUpdatePromptOpen(true)}
                      className="flex animate-pulse items-center gap-1 rounded-full border border-sky-400/40 bg-sky-500/20 px-2.5 py-0.5 text-[10px] font-bold text-sky-300 transition hover:bg-sky-500/30"
                      title="Open update release notes"
                    >
                      <DownloadCloud className="h-3 w-3" />
                      Update v{updateInfo.latestVersion}
                    </button>
                  )}
                </div>
              )}
            </div>

            <div className="flex shrink-0 items-center gap-1.5">
              <button
                type="button"
                onClick={() => void hideMenu()}
                className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-2.5 py-1 text-[11px] font-bold text-[var(--accent-fg)] shadow-[0_0_14px_var(--accent-glow)] transition hover:opacity-90"
                title="Hide menu (overlay stays visible)"
              >
                <EyeOff className="h-3.5 w-3.5" />
                <span className="hidden sm:inline">Hide ({ui.hotkey})</span>
              </button>
              <button
                type="button"
                onClick={() => void minimizeMenu()}
                style={{
                  backgroundColor: "var(--surface-subtle)",
                  borderColor: "var(--border-subtle)",
                }}
                className="rounded-lg border p-1.5 opacity-75 transition hover:opacity-100"
                title="Minimize"
              >
                <Minus className="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                onClick={() => void handleToggleMaximize()}
                style={{
                  backgroundColor: "var(--surface-subtle)",
                  borderColor: "var(--border-subtle)",
                }}
                className="rounded-lg border p-1.5 opacity-75 transition hover:opacity-100"
                title={maximized ? "Restore" : "Maximize"}
              >
                {maximized ? (
                  <Copy className="h-3.5 w-3.5" />
                ) : (
                  <Maximize2 className="h-3.5 w-3.5" />
                )}
              </button>
              <button
                type="button"
                onClick={() => void quitApp()}
                className="rounded-lg border border-red-500/25 bg-red-500/10 p-1.5 text-red-400 transition hover:bg-red-500/20"
                title="Quit Luair"
              >
                <Power className="h-3.5 w-3.5" />
              </button>
            </div>
          </header>

          {!authLoaded ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3">
              <LuairLogo size={52} />
              <span className="text-xs font-semibold tracking-wider text-[var(--muted-text)] uppercase">
                Unlocking Local Vault...
              </span>
            </div>
          ) : !isReadyInStudio ? (
            <OnboardingFlow
              session={session}
              theme={theme}
              profiles={profiles}
              currentHotkey={ui.hotkey}
              currentShield={ui.streamproof}
              onPreviewTheme={(nextTheme) => handleThemeChange(nextTheme, false)}
              onSessionChange={(nextSession) => setSession(nextSession)}
            />
          ) : (
            <div className="flex min-h-0 flex-1">
              <aside
                style={{
                  borderColor: "var(--border-subtle)",
                  backgroundColor: "var(--surface-subtle)",
                }}
                className="flex w-16 shrink-0 flex-col justify-between border-r p-2.5 transition-[width] duration-200 sm:w-52 sm:p-3"
              >
                <nav className="space-y-1">
                  {navItems.map((item) => {
                    const Icon = item.icon;
                    const active = tab === item.id;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => setTab(item.id)}
                        style={{
                          color: active ? "var(--accent-fg)" : "var(--text-color)",
                        }}
                        className={`relative z-10 flex w-full items-center justify-center gap-2.5 rounded-xl px-2.5 py-2.5 text-left text-xs font-bold transition-colors sm:justify-start sm:px-3 ${
                          active ? "" : "opacity-75 hover:opacity-100"
                        }`}
                      >
                        {active && (
                          <motion.span
                            layoutId="active-nav-pill"
                            transition={{ type: "spring", stiffness: 420, damping: 32 }}
                            className="absolute inset-0 -z-10 rounded-xl bg-[var(--accent)] shadow-[0_0_20px_var(--accent-glow)]"
                          />
                        )}
                        <Icon className="h-4 w-4 shrink-0" />
                        <span className="hidden truncate sm:inline">{item.label}</span>
                      </button>
                    );
                  })}
                </nav>

                <div className="space-y-2">
                  <div
                    style={{
                      backgroundColor: "var(--surface-input)",
                      borderColor: "var(--border-subtle)",
                    }}
                    className="hidden space-y-2 rounded-xl border p-3 text-[10px] sm:block"
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-[var(--muted-text)]">Toggle Menu</span>
                      <span className="font-mono font-bold text-[var(--accent)]">
                        {ui.hotkey}
                      </span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-[var(--muted-text)]">ADS Quick-Hide</span>
                      <span className="font-mono font-bold text-[var(--accent)]">
                        {ui.adsHotkey ?? "None"}
                      </span>
                    </div>
                    <div
                      style={{ borderColor: "var(--border-subtle)" }}
                      className="border-t pt-2 text-[10px] text-[var(--muted-text)]"
                    >
                      {theme.name} ·{" "}
                      {theme.surfaceMode === "liquid-glass" ? "Liquid Glass" : "Solid"}
                    </div>
                  </div>

                  {/* Signed-in Local Account Chip */}
                  <div
                    style={{
                      backgroundColor: "var(--surface-input)",
                      borderColor: "var(--border-subtle)",
                    }}
                    className="flex items-center justify-between gap-2 rounded-xl border p-2 text-[10px]"
                  >
                    <div className="hidden min-w-0 sm:block">
                      <div className="truncate font-bold">
                        @{session.account?.username}
                      </div>
                      <div className="truncate text-[9px] text-[var(--muted-text)]">
                        Local Vault
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => void handleSignOut()}
                      className="mx-auto rounded-lg border border-rose-500/20 bg-rose-500/10 p-1.5 text-rose-300 transition hover:bg-rose-500/20 sm:mx-0"
                      title="Sign Out"
                    >
                      <LogOut className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              </aside>

              <main className="luair-scroll min-w-0 flex-1 overflow-y-auto p-4 text-xs sm:p-5">
                <AnimatePresence mode="wait">
                  <motion.div
                    key={tab}
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.16, ease: "easeOut" }}
                  >
                    {tab === "crosshair" && (
                      <CrosshairTab
                        crosshair={crosshair}
                        activeProfileId={activeProfileId}
                        surfaceMode={theme.surfaceMode}
                        onUpdate={updateCrosshair}
                        onReset={() =>
                          commitState(
                            (prev) => ({
                              ...prev,
                              crosshair: { ...DEFAULT_CROSSHAIR_CONFIG },
                            }),
                            true,
                          )
                        }
                        onAiApplied={(nextState) => setState(nextState)}
                      />
                    )}

                    {tab === "profiles" && (
                      <ProfilesTab
                        profiles={profiles}
                        activeProfileId={activeProfileId}
                        currentCrosshair={crosshair}
                        enableProfileHotkeys={ui.enableProfileHotkeys}
                        recordingProfileId={
                          recording?.kind === "profile" ? recording.profileId : null
                        }
                        switchedBanner={switchedBanner}
                        surfaceMode={theme.surfaceMode}
                        onToggleProfileHotkeys={(enabled) =>
                          commitState(
                            (prev) => ({
                              ...prev,
                              ui: { ...prev.ui, enableProfileHotkeys: enabled },
                            }),
                            true,
                          )
                        }
                        onAddProfile={handleAddProfile}
                        onSelectProfile={(id) => void selectProfile(id)}
                        onSaveOverProfile={handleSaveOverProfile}
                        onRenameProfile={handleRenameProfile}
                        onDeleteProfile={handleDeleteProfile}
                        onStartRecording={(profileId) =>
                          setRecording({ kind: "profile", profileId })
                        }
                        onManualBind={handleManualBind}
                        onImportCrosshair={handleImportCrosshair}
                        onImportAsProfile={handleImportAsProfile}
                      />
                    )}

                    {tab === "shield" && (
                      <ShieldTab
                        streamproof={ui.streamproof}
                        cloakMenu={ui.cloakMenu}
                        cloakStatus={cloakStatus}
                        surfaceMode={theme.surfaceMode}
                        onToggleCloak={(enabled, cloakMenu) =>
                          void handleToggleCloak(enabled, cloakMenu)
                        }
                      />
                    )}

                    {tab === "theme" && (
                      <ThemeTab
                        theme={theme}
                        onChangeTheme={handleThemeChange}
                        onAiApplied={(nextState) => setState(nextState)}
                      />
                    )}

                    {tab === "ai" && (
                      <AiAssistantTab
                        surfaceMode={theme.surfaceMode}
                        onStateApplied={(nextState) => setState(nextState)}
                      />
                    )}

                    {tab === "settings" && (
                      <SettingsTab
                        menuHotkey={ui.hotkey}
                        adsHotkey={ui.adsHotkey}
                        recordingKind={
                          recording?.kind === "menu" || recording?.kind === "ads"
                            ? recording.kind
                            : null
                        }
                        session={session}
                        surfaceMode={theme.surfaceMode}
                        updateInfo={updateInfo}
                        checkingUpdate={checkingUpdate}
                        platformInfo={platformInfo}
                        onStartRecording={(kind) => setRecording({ kind })}
                        onResetOverlayBounds={() => void resetOverlayBounds()}
                        onReplayOnboarding={() => void handleReplayOnboarding()}
                        onSignOut={() => void handleSignOut()}
                        onCheckUpdates={() => void runUpdateCheck(true)}
                        onOpenUpdateModal={() => setUpdatePromptOpen(true)}
                      />
                    )}
                  </motion.div>
                </AnimatePresence>
              </main>
            </div>
          )}

          {updatePromptOpen && updateInfo?.available && (
            <UpdateBanner
              update={updateInfo}
              progress={updateProgress}
              installing={installingUpdate}
              error={updateError}
              onUpdateNow={() => void handleInstallUpdate()}
              onDismiss={() => {
                if (!installingUpdate) setUpdatePromptOpen(false);
              }}
            />
          )}
        </DynamicGlassShell>
      </div>
    </MotionConfig>
  );
}
