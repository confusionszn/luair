import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  DEFAULT_AUTH_SESSION,
  type AccountProfile,
  type AuthSession,
} from "../types/auth";
import type {
  AiApplyResult,
  AiChatResponse,
  AiDownloadProgress,
  AiEngineStatus,
  ChatMessage,
} from "../types/ai";
import {
  DEFAULT_LUAIR_STATE,
  parseCrosshairConfig,
  type CloakStatus,
  type LuairState,
} from "../types/crosshair";
import type { ThemeConfig } from "../types/themes";
import type {
  PlatformBuildInfo,
  UpdateCheckResult,
  UpdateProgressEvent,
} from "../types/updater";
import { normalizeThemeConfig } from "./themeCodec";

const BROWSER_FALLBACK_KEY = "luair_v2_state";
const BROWSER_VAULT_KEY = "luair_v2_mock_vault";

export type WindowMode = "overlay" | "menu";
export type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

interface BrowserVaultRecord {
  account: AccountProfile;
  password: string;
  state: LuairState;
}

interface BrowserVaultStore {
  accounts: BrowserVaultRecord[];
  rememberedAccountId: string | null;
  activeAccountId: string | null;
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function getWindowMode(): WindowMode {
  if (typeof window === "undefined") {
    return "menu";
  }
  const param = new URLSearchParams(window.location.search).get("window");
  return param === "overlay" ? "overlay" : "menu";
}

function hydrateState(parsed: Partial<LuairState>): LuairState {
  const profiles = Array.isArray(parsed.profiles)
    ? parsed.profiles.map((p) => ({
        ...p,
        config: parseCrosshairConfig(p.config),
      }))
    : DEFAULT_LUAIR_STATE.profiles.map((p) => ({
        ...p,
        config: { ...p.config },
      }));

  return {
    ...DEFAULT_LUAIR_STATE,
    ...parsed,
    crosshair: parseCrosshairConfig(parsed.crosshair ?? DEFAULT_LUAIR_STATE.crosshair),
    ui: {
      ...DEFAULT_LUAIR_STATE.ui,
      ...parsed.ui,
      theme: normalizeThemeConfig(parsed.ui?.theme),
    },
    profiles,
  };
}

function readBrowserVault(): BrowserVaultStore {
  try {
    const raw = localStorage.getItem(BROWSER_VAULT_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<BrowserVaultStore>;
      return {
        accounts: Array.isArray(parsed.accounts) ? parsed.accounts : [],
        rememberedAccountId: parsed.rememberedAccountId ?? null,
        activeAccountId: parsed.activeAccountId ?? parsed.rememberedAccountId ?? null,
      };
    }
  } catch {
    // Ignore corrupt preview vault.
  }
  return { accounts: [], rememberedAccountId: null, activeAccountId: null };
}

function writeBrowserVault(store: BrowserVaultStore): void {
  localStorage.setItem(BROWSER_VAULT_KEY, JSON.stringify(store));
}

function browserAuthSession(store: BrowserVaultStore): AuthSession {
  const id = store.activeAccountId ?? store.rememberedAccountId;
  const found = id ? store.accounts.find((a) => a.account.id === id) : undefined;
  return {
    authenticated: Boolean(found),
    rememberMe: Boolean(found && store.rememberedAccountId === found.account.id),
    account: found ? found.account : null,
    hasExistingAccounts: store.accounts.length > 0,
    vaultEngine: "Browser Preview Mock Vault (Desktop uses SQLite + AES-256-GCM)",
  };
}

function readBrowserState(): LuairState {
  const vault = readBrowserVault();
  const activeId = vault.activeAccountId ?? vault.rememberedAccountId;
  const activeRecord = activeId
    ? vault.accounts.find((a) => a.account.id === activeId)
    : undefined;
  if (activeRecord) {
    return hydrateState(activeRecord.state);
  }
  try {
    const raw = localStorage.getItem(BROWSER_FALLBACK_KEY);
    if (raw) {
      return hydrateState(JSON.parse(raw) as Partial<LuairState>);
    }
  } catch {
    // Ignore malformed browser fallback state.
  }
  return hydrateState({});
}

function writeBrowserState(state: LuairState): void {
  localStorage.setItem(BROWSER_FALLBACK_KEY, JSON.stringify(state));
  const vault = readBrowserVault();
  const activeId = vault.activeAccountId ?? vault.rememberedAccountId;
  if (activeId) {
    const idx = vault.accounts.findIndex((a) => a.account.id === activeId);
    if (idx !== -1) {
      vault.accounts[idx].state = state;
      writeBrowserVault(vault);
    }
  }
}

export async function getState(): Promise<LuairState> {
  if (!isTauriRuntime()) {
    return readBrowserState();
  }
  const remote = await invoke<Partial<LuairState>>("get_state");
  return hydrateState(remote);
}

export async function updateState(state: LuairState): Promise<LuairState> {
  if (!isTauriRuntime()) {
    writeBrowserState(state);
    window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: state }));
    return state;
  }
  return invoke<LuairState>("update_state", { state });
}

export async function selectProfile(profileId: string): Promise<LuairState> {
  if (!isTauriRuntime()) {
    const current = readBrowserState();
    const target = current.profiles.find((p) => p.id === profileId);
    if (!target) return current;
    const next: LuairState = {
      ...current,
      crosshair: { ...target.config },
      activeProfileId: target.id,
    };
    writeBrowserState(next);
    window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: next }));
    return next;
  }
  return invoke<LuairState>("select_profile", { profileId });
}

export async function getAuthSession(): Promise<AuthSession> {
  if (!isTauriRuntime()) {
    return browserAuthSession(readBrowserVault());
  }
  return invoke<AuthSession>("get_auth_session");
}

export async function signUpLocal(payload: {
  username: string;
  email: string;
  password: string;
  confirmPassword: string;
  rememberMe: boolean;
}): Promise<AuthSession> {
  if (!isTauriRuntime()) {
    const username = payload.username.trim();
    const email = payload.email.trim();
    if (username.length < 3 || username.length > 24) {
      throw new Error("Username must be between 3 and 24 characters.");
    }
    if (!email.includes("@") || !email.includes(".")) {
      throw new Error("Please enter a valid email address.");
    }
    if (payload.password.length < 8) {
      throw new Error("Password must be at least 8 characters long.");
    }
    if (payload.password !== payload.confirmPassword) {
      throw new Error("Passwords do not match.");
    }
    const store = readBrowserVault();
    if (
      store.accounts.some(
        (a) => a.account.username.toLowerCase() === username.toLowerCase(),
      )
    ) {
      throw new Error("That username is already registered in this local vault.");
    }
    if (
      store.accounts.some((a) => a.account.email.toLowerCase() === email.toLowerCase())
    ) {
      throw new Error("That email is already registered in this local vault.");
    }
    const now = Math.floor(Date.now() / 1000);
    const account: AccountProfile = {
      id: `local-${Date.now()}`,
      username,
      email,
      createdAt: now,
      lastLoginAt: now,
      onboardingCompleted: false,
    };
    const nextState = hydrateState({});
    store.accounts.push({ account, password: payload.password, state: nextState });
    store.activeAccountId = account.id;
    store.rememberedAccountId = payload.rememberMe ? account.id : null;
    writeBrowserVault(store);
    const session = browserAuthSession(store);
    window.dispatchEvent(new CustomEvent("luair-browser-auth", { detail: session }));
    window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: nextState }));
    return session;
  }

  return invoke<AuthSession>("sign_up_local", payload);
}

export async function signInLocal(payload: {
  identifier: string;
  password: string;
  rememberMe: boolean;
}): Promise<AuthSession> {
  if (!isTauriRuntime()) {
    const ident = payload.identifier.trim().toLowerCase();
    const store = readBrowserVault();
    const idx = store.accounts.findIndex(
      (a) =>
        a.account.username.toLowerCase() === ident ||
        a.account.email.toLowerCase() === ident,
    );
    if (idx === -1 || store.accounts[idx].password !== payload.password) {
      throw new Error("Invalid local credentials.");
    }
    store.accounts[idx].account.lastLoginAt = Math.floor(Date.now() / 1000);
    store.activeAccountId = store.accounts[idx].account.id;
    store.rememberedAccountId = payload.rememberMe ? store.activeAccountId : null;
    writeBrowserVault(store);
    const session = browserAuthSession(store);
    window.dispatchEvent(new CustomEvent("luair-browser-auth", { detail: session }));
    window.dispatchEvent(
      new CustomEvent("luair-browser-state", { detail: store.accounts[idx].state }),
    );
    return session;
  }

  return invoke<AuthSession>("sign_in_local", payload);
}

export async function signOutLocal(): Promise<AuthSession> {
  if (!isTauriRuntime()) {
    const store = readBrowserVault();
    store.activeAccountId = null;
    store.rememberedAccountId = null;
    writeBrowserVault(store);
    const session = browserAuthSession(store);
    window.dispatchEvent(new CustomEvent("luair-browser-auth", { detail: session }));
    return session;
  }
  return invoke<AuthSession>("sign_out_local");
}

export async function completeOnboarding(payload: {
  starterProfileId?: string;
  theme?: ThemeConfig;
  enableShield?: boolean;
  menuHotkey?: string;
}): Promise<AuthSession> {
  if (!isTauriRuntime()) {
    const store = readBrowserVault();
    const idx = store.accounts.findIndex((a) => a.account.id === store.activeAccountId);
    if (idx === -1) return DEFAULT_AUTH_SESSION;
    const nextState = hydrateState(store.accounts[idx].state);
    if (payload.starterProfileId) {
      const profile = nextState.profiles.find((p) => p.id === payload.starterProfileId);
      if (profile) {
        nextState.crosshair = { ...profile.config };
        nextState.activeProfileId = profile.id;
      }
    }
    if (payload.theme) {
      nextState.ui.uiTheme = payload.theme.name;
      nextState.ui.uiAccent = payload.theme.accent;
      nextState.ui.theme = payload.theme;
    }
    if (typeof payload.enableShield === "boolean") {
      nextState.ui.streamproof = payload.enableShield;
    }
    if (payload.menuHotkey?.trim()) {
      nextState.ui.hotkey = payload.menuHotkey.trim();
    }
    store.accounts[idx].account.onboardingCompleted = true;
    store.accounts[idx].state = nextState;
    writeBrowserVault(store);
    const session = browserAuthSession(store);
    window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: nextState }));
    window.dispatchEvent(new CustomEvent("luair-browser-auth", { detail: session }));
    return session;
  }

  return invoke<AuthSession>("complete_onboarding", payload);
}

export async function reopenOnboarding(): Promise<AuthSession> {
  if (!isTauriRuntime()) {
    const store = readBrowserVault();
    const idx = store.accounts.findIndex((a) => a.account.id === store.activeAccountId);
    if (idx === -1) return DEFAULT_AUTH_SESSION;
    store.accounts[idx].account.onboardingCompleted = false;
    writeBrowserVault(store);
    const session = browserAuthSession(store);
    window.dispatchEvent(new CustomEvent("luair-browser-auth", { detail: session }));
    return session;
  }
  return invoke<AuthSession>("reopen_onboarding");
}

export async function setCaptureCloak(
  enabled: boolean,
  cloakMenu?: boolean,
): Promise<CloakStatus> {
  if (!isTauriRuntime()) {
    const current = readBrowserState();
    const nextCloakMenu = cloakMenu ?? current.ui.cloakMenu;
    const next: LuairState = {
      ...current,
      ui: { ...current.ui, streamproof: enabled, cloakMenu: nextCloakMenu },
    };
    writeBrowserState(next);
    window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: next }));
    return {
      enabled,
      cloakMenu: nextCloakMenu,
      platform: "browser-preview",
      mechanism: "WDA_EXCLUDEFROMCAPTURE (0x11) [Simulated in browser]",
      supported: false,
      overlayVerified: false,
    };
  }
  return invoke<CloakStatus>("set_capture_cloak", { enabled, cloakMenu });
}

export async function getCaptureCloakStatus(): Promise<CloakStatus> {
  if (!isTauriRuntime()) {
    const current = readBrowserState();
    return {
      enabled: current.ui.streamproof,
      cloakMenu: current.ui.cloakMenu,
      platform: "browser-preview",
      mechanism: "WDA_EXCLUDEFROMCAPTURE (0x11) [Simulated in browser]",
      supported: false,
      overlayVerified: false,
    };
  }
  return invoke<CloakStatus>("get_capture_cloak_status");
}

export async function toggleMenu(): Promise<boolean> {
  if (!isTauriRuntime()) return true;
  return invoke<boolean>("toggle_menu");
}

export async function hideMenu(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("hide_menu");
}

export async function minimizeMenu(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("minimize_menu");
}

export async function toggleMaximizeMenu(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  const win = getCurrentWindow();
  await win.toggleMaximize();
  return win.isMaximized();
}

export async function isMenuMaximized(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  return getCurrentWindow().isMaximized();
}

export async function startResizeDragging(direction: ResizeDirection): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().startResizeDragging(direction);
}

export async function resetOverlayBounds(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("reset_overlay_bounds");
}

export async function quitApp(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("quit_app");
}

export async function setHotkeyRecording(recording: boolean): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("set_hotkey_recording", { recording });
}

export async function onStateUpdated(
  callback: (state: LuairState) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    const handler = (event: Event) => {
      const custom = event as CustomEvent<LuairState>;
      if (custom.detail) callback(hydrateState(custom.detail));
    };
    window.addEventListener("luair-browser-state", handler);
    return () => window.removeEventListener("luair-browser-state", handler);
  }

  return listen<Partial<LuairState>>("luair://state-updated", (event) => {
    callback(hydrateState(event.payload));
  });
}

export async function onAuthUpdated(
  callback: (session: AuthSession) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    const handler = (event: Event) => {
      const custom = event as CustomEvent<AuthSession>;
      if (custom.detail) callback(custom.detail);
    };
    window.addEventListener("luair-browser-auth", handler);
    return () => window.removeEventListener("luair-browser-auth", handler);
  }

  return listen<AuthSession>("luair://auth-updated", (event) => {
    callback(event.payload);
  });
}

export async function onProfileSwitched(
  callback: (profileId: string) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => {};
  }

  return listen<string>("luair://profile-switched", (event) => {
    callback(event.payload);
  });
}

export async function onFireState(
  callback: (firing: boolean) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => {};
  }

  return listen<boolean>("luair://fire-state", (event) => {
    callback(Boolean(event.payload));
  });
}

export async function onMouseCaptured(
  callback: (binding: string) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => {};
  }

  return listen<string>("luair://mouse-captured", (event) => {
    callback(event.payload);
  });
}

const BROWSER_AI_MOCK_MODELS = [
  {
    id: "smollm2-360m",
    name: "SmolLM2-360M-Instruct",
    architecture: "llama",
    quantization: "Q4_K_M",
    approxSizeMb: 258,
    description:
      "Ultra-lightweight 360M Llama-family model for fast CPU chat & JSON synthesis.",
    downloaded: false,
    loaded: false,
    localSizeBytes: 0,
  },
  {
    id: "qwen2.5-0.5b",
    name: "Qwen2.5-0.5B-Instruct",
    architecture: "qwen2",
    quantization: "Q4_K_M",
    approxSizeMb: 469,
    description:
      "High-accuracy 0.5B Qwen2.5 instruction model with strong structured JSON output.",
    downloaded: false,
    loaded: false,
    localSizeBytes: 0,
  },
];

function browserAiPreviewCrosshair(current: LuairState, prompt: string): AiApplyResult {
  const lower = prompt.toLowerCase();
  const nextCrosshair = { ...current.crosshair };
  if (lower.includes("lime")) nextCrosshair.color = "#84cc16";
  else if (lower.includes("purple")) nextCrosshair.color = "#a855f7";
  else if (lower.includes("red") || lower.includes("crimson")) nextCrosshair.color = "#f43f5e";
  else if (lower.includes("cyan")) nextCrosshair.color = "#22d3ee";

  if (lower.includes("dot")) {
    nextCrosshair.dot = true;
    nextCrosshair.hideBars = !lower.includes("cross") && !lower.includes("t-shape");
  }
  if (lower.includes("thin border")) {
    nextCrosshair.outline = true;
    nextCrosshair.outlineThickness = 1;
  }
  const keyMatch = /\b(?:key|slot)\s+([0-9])\b/.exec(lower);
  const boundHotkey = keyMatch ? keyMatch[1] : null;
  const profiles = current.profiles.map((p, idx) =>
    boundHotkey && String(idx + 1) === boundHotkey
      ? { ...p, hotkey: boundHotkey, config: nextCrosshair }
      : p,
  );
  const nextState: LuairState = {
    ...current,
    crosshair: nextCrosshair,
    profiles,
  };
  writeBrowserState(nextState);
  window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: nextState }));
  return {
    summary: `Generated crosshair (${nextCrosshair.color})${boundHotkey ? ` on [${boundHotkey}]` : ""}.`,
    engineUsed: "Browser Preview Fallback",
    boundHotkey,
    state: nextState,
  };
}

function browserAiPreviewTheme(current: LuairState, prompt: string): AiApplyResult {
  const lower = prompt.toLowerCase();
  const nextTheme = { ...current.ui.theme, id: "ai-custom", name: "AI Studio Theme" };
  if (lower.includes("solid") || lower.includes("minimal")) {
    nextTheme.surfaceMode = "solid-minimal";
    nextTheme.blurPx = 0;
  } else if (lower.includes("liquid") || lower.includes("glass")) {
    nextTheme.surfaceMode = "liquid-glass";
    nextTheme.blurPx = 22;
  }
  if (lower.includes("purple") || lower.includes("violet")) {
    nextTheme.background = "#0d061a";
    nextTheme.surface = "#190c30";
    nextTheme.accent = "#a855f7";
    nextTheme.highlight = "#f472b6";
  }
  const nextState: LuairState = {
    ...current,
    ui: { ...current.ui, theme: nextTheme, uiTheme: nextTheme.name, uiAccent: nextTheme.accent },
  };
  writeBrowserState(nextState);
  window.dispatchEvent(new CustomEvent("luair-browser-state", { detail: nextState }));
  return {
    summary: `Applied AI ${nextTheme.surfaceMode} theme (${nextTheme.accent}).`,
    engineUsed: "Browser Preview Fallback",
    boundHotkey: null,
    state: nextState,
  };
}

export async function getAiStatus(): Promise<AiEngineStatus> {
  if (!isTauriRuntime()) {
    return {
      models: BROWSER_AI_MOCK_MODELS,
      loadedModelId: null,
      downloadingModelId: null,
      activeBackend: "Browser Preview Fallback (Desktop runs Candle GGUF)",
    };
  }
  return invoke<AiEngineStatus>("get_ai_status");
}

export async function downloadAiModel(modelId: string): Promise<AiEngineStatus> {
  if (!isTauriRuntime()) {
    throw new Error("GGUF model downloading requires the desktop Tauri runtime.");
  }
  return invoke<AiEngineStatus>("download_ai_model", { modelId });
}

export async function cancelAiDownload(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cancel_ai_download");
}

export async function loadAiModel(modelId: string): Promise<AiEngineStatus> {
  if (!isTauriRuntime()) {
    throw new Error("GGUF model inference requires the desktop Tauri runtime.");
  }
  return invoke<AiEngineStatus>("load_ai_model", { modelId });
}

export async function unloadAiModel(): Promise<AiEngineStatus> {
  if (!isTauriRuntime()) return getAiStatus();
  return invoke<AiEngineStatus>("unload_ai_model");
}

export async function deleteAiModel(modelId: string): Promise<AiEngineStatus> {
  if (!isTauriRuntime()) return getAiStatus();
  return invoke<AiEngineStatus>("delete_ai_model", { modelId });
}

export async function aiMakeCrosshair(
  prompt: string,
  targetSlotId?: string | null,
): Promise<AiApplyResult> {
  if (!isTauriRuntime()) {
    return browserAiPreviewCrosshair(readBrowserState(), prompt);
  }
  return invoke<AiApplyResult>("ai_make_crosshair", {
    prompt,
    targetSlotId: targetSlotId ?? null,
  });
}

export async function aiMakeTheme(prompt: string): Promise<AiApplyResult> {
  if (!isTauriRuntime()) {
    return browserAiPreviewTheme(readBrowserState(), prompt);
  }
  return invoke<AiApplyResult>("ai_make_theme", { prompt });
}

export async function aiChat(
  messages: ChatMessage[],
  prompt: string,
): Promise<AiChatResponse> {
  if (!isTauriRuntime()) {
    const lower = prompt.toLowerCase();
    if (lower.includes("theme") && lower.includes("make")) {
      const res = browserAiPreviewTheme(readBrowserState(), prompt);
      return {
        reply: res.summary,
        engineUsed: res.engineUsed,
        appliedAction: res.summary,
        updatedState: res.state,
      };
    }
    if ((lower.includes("crosshair") || lower.includes("dot")) && lower.includes("make")) {
      const res = browserAiPreviewCrosshair(readBrowserState(), prompt);
      return {
        reply: res.summary,
        engineUsed: res.engineUsed,
        appliedAction: res.summary,
        updatedState: res.state,
      };
    }
    return {
      reply:
        "Browser preview mode active. Run `pnpm.cmd tauri dev` to download and run Candle GGUF models locally in Rust.",
      engineUsed: "Browser Preview Fallback",
      appliedAction: null,
      updatedState: null,
    };
  }
  return invoke<AiChatResponse>("ai_chat", { messages, prompt });
}

export async function onAiDownloadProgress(
  callback: (progress: AiDownloadProgress) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => {};
  }
  return listen<AiDownloadProgress>("luair://ai-download-progress", (event) => {
    callback(event.payload);
  });
}

export async function checkForUpdates(): Promise<UpdateCheckResult> {
  if (!isTauriRuntime()) {
    return {
      available: false,
      currentVersion: "2.0.2",
      latestVersion: "2.0.2",
      releaseNotes: null,
      releaseDate: null,
      releaseUrl: "https://github.com/confusionszn/luair/releases",
      endpoint:
        "https://github.com/confusionszn/luair/releases/latest/download/latest.json",
      status: "up-to-date",
      message: "Browser Preview mode: Tauri Updater active on desktop builds.",
    };
  }
  return invoke<UpdateCheckResult>("check_for_updates");
}

export async function downloadAndInstallUpdate(): Promise<void> {
  if (!isTauriRuntime()) {
    throw new Error("Auto-update installation requires the native Tauri desktop app.");
  }
  await invoke("download_and_install_update");
}

export async function getPlatformInfo(): Promise<PlatformBuildInfo> {
  if (!isTauriRuntime()) {
    return {
      appVersion: "2.0.2",
      os: "browser-preview",
      arch: "wasm-web",
      targetTriple: "browser-preview",
      displayServer: "Browser DOM / Canvas Preview",
      overlayBackend: "Desktop builds use WebView2 / GTK3 / WKWebView",
      packagingTargets: [
        "Windows MSVC x86_64 (NSIS & MSI)",
        "Linux Debian (.deb)",
        "Linux Arch (PKGBUILD)",
        "Linux Fedora (.rpm)",
        "Linux Flatpak & AppImage (X11/Wayland)",
        "macOS Universal Binary (x86_64 + arm64 DMG)",
      ],
      updaterEndpoint:
        "https://github.com/confusionszn/luair/releases/latest/download/latest.json",
    };
  }
  return invoke<PlatformBuildInfo>("get_platform_info");
}

export async function onUpdateProgress(
  callback: (progress: UpdateProgressEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => {};
  }
  return listen<UpdateProgressEvent>("luair://update-progress", (event) => {
    callback(event.payload);
  });
}

export async function onTriggerUpdateCheck(callback: () => void): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => {};
  }
  return listen("luair://trigger-update-check", () => {
    callback();
  });
}
