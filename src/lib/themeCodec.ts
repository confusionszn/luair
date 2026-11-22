import {
  DEFAULT_THEME_CONFIG,
  isLightColor,
  type ThemeConfig,
  type ThemeSurfaceMode,
} from "../types/themes";

export const THEME_CODE_PREFIX = "LUAIR1:";

interface ExportedThemeEnvelope extends ThemeConfig {
  schema: "luair-theme";
  version: 1;
}

export type ThemeImportResult =
  | { ok: true; theme: ThemeConfig; format: "code" | "json" }
  | { ok: false; error: string };

function normalizeHex(value: unknown, fallback: string): string {
  if (typeof value !== "string") return fallback;
  const trimmed = value.trim();
  const matchShort = /^#([0-9a-fA-F]{3})$/.exec(trimmed);
  if (matchShort) {
    const [r, g, b] = matchShort[1].split("");
    return `#${r}${r}${g}${g}${b}${b}`.toLowerCase();
  }
  if (/^#[0-9a-fA-F]{6}$/.test(trimmed)) {
    return trimmed.toLowerCase();
  }
  return fallback;
}

function clampNumber(value: unknown, fallback: number, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, value));
}

function normalizeSurfaceMode(value: unknown, fallback: ThemeSurfaceMode): ThemeSurfaceMode {
  if (value === "liquid-glass" || value === "solid-minimal") {
    return value;
  }
  return fallback;
}

export function normalizeThemeConfig(raw: unknown): ThemeConfig {
  if (typeof raw !== "object" || raw === null) {
    return { ...DEFAULT_THEME_CONFIG };
  }
  const src = raw as Record<string, unknown>;
  const background = normalizeHex(src.background, DEFAULT_THEME_CONFIG.background);
  const light = isLightColor(background);

  const name =
    typeof src.name === "string" && src.name.trim().length > 0
      ? src.name.trim().slice(0, 48)
      : "Custom Theme";

  const id =
    typeof src.id === "string" && src.id.trim().length > 0
      ? src.id.trim().slice(0, 48)
      : name.toLowerCase().replace(/[^a-z0-9]+/g, "-") || "custom-theme";

  return {
    id,
    name,
    surfaceMode: normalizeSurfaceMode(src.surfaceMode, DEFAULT_THEME_CONFIG.surfaceMode),
    background,
    surface: normalizeHex(src.surface, DEFAULT_THEME_CONFIG.surface),
    text: normalizeHex(src.text, light ? "#0b1d33" : "#eef4ff"),
    mutedText: normalizeHex(src.mutedText, light ? "#4a6482" : "#8a99bd"),
    accent: normalizeHex(src.accent, DEFAULT_THEME_CONFIG.accent),
    highlight: normalizeHex(src.highlight, DEFAULT_THEME_CONFIG.highlight),
    blurPx: Math.round(clampNumber(src.blurPx, DEFAULT_THEME_CONFIG.blurPx, 0, 36)),
    glassOpacity: Number(
      clampNumber(src.glassOpacity, DEFAULT_THEME_CONFIG.glassOpacity, 0.35, 1).toFixed(2),
    ),
    borderRefraction: Math.round(
      clampNumber(src.borderRefraction, DEFAULT_THEME_CONFIG.borderRefraction, 0, 100),
    ),
    dynamicLighting:
      typeof src.dynamicLighting === "boolean"
        ? src.dynamicLighting
        : DEFAULT_THEME_CONFIG.dynamicLighting,
    cornerRadius: Math.round(
      clampNumber(src.cornerRadius, DEFAULT_THEME_CONFIG.cornerRadius, 0, 24),
    ),
  };
}

function bytesToBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlToBytes(input: string): Uint8Array {
  const padded = input.replace(/-/g, "+").replace(/_/g, "/");
  const mod = padded.length % 4;
  const full = mod === 0 ? padded : padded + "=".repeat(4 - mod);
  const binary = atob(full);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function encodeThemeCode(theme: ThemeConfig): string {
  const clean = normalizeThemeConfig(theme);
  const json = JSON.stringify(clean);
  const encoded = bytesToBase64Url(new TextEncoder().encode(json));
  return `${THEME_CODE_PREFIX}${encoded}`;
}

export function exportThemeJson(theme: ThemeConfig): string {
  const clean = normalizeThemeConfig(theme);
  const envelope: ExportedThemeEnvelope = {
    schema: "luair-theme",
    version: 1,
    ...clean,
  };
  return JSON.stringify(envelope, null, 2);
}

export function importThemeInput(raw: string): ThemeImportResult {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { ok: false, error: "Paste a LUAIR1: share code or JSON theme payload." };
  }

  if (trimmed.toUpperCase().startsWith(THEME_CODE_PREFIX)) {
    const payload = trimmed.slice(THEME_CODE_PREFIX.length).trim();
    if (!payload) {
      return { ok: false, error: "Share code is empty after LUAIR1: prefix." };
    }
    try {
      const jsonStr = new TextDecoder().decode(base64UrlToBytes(payload));
      const parsed: unknown = JSON.parse(jsonStr);
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        return { ok: false, error: "Decoded share code does not contain a theme object." };
      }
      return { ok: true, theme: normalizeThemeConfig(parsed), format: "code" };
    } catch {
      return { ok: false, error: "Invalid or corrupted LUAIR1: share code." };
    }
  }

  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return { ok: false, error: "Theme JSON must be a valid JSON object." };
    }
    const obj = parsed as Record<string, unknown>;
    const hasThemeField =
      "accent" in obj || "background" in obj || "surfaceMode" in obj || "surface" in obj;
    if (!hasThemeField) {
      return {
        ok: false,
        error: "JSON object is missing theme color or surface fields (accent, background, surfaceMode).",
      };
    }
    return { ok: true, theme: normalizeThemeConfig(obj), format: "json" };
  } catch {
    return { ok: false, error: "Invalid JSON syntax or unrecognized share code." };
  }
}
