export type ThemeSurfaceMode = "liquid-glass" | "solid-minimal";

export interface ThemeConfig {
  id: string;
  name: string;
  surfaceMode: ThemeSurfaceMode;
  background: string;
  surface: string;
  text: string;
  mutedText: string;
  accent: string;
  highlight: string;
  blurPx: number;
  glassOpacity: number;
  borderRefraction: number;
  dynamicLighting: boolean;
  cornerRadius: number;
}

export const THEME_PRESETS: ThemeConfig[] = [
  {
    id: "deep-space",
    name: "Deep Space",
    surfaceMode: "liquid-glass",
    background: "#070913",
    surface: "#101528",
    text: "#eef4ff",
    mutedText: "#8a99bd",
    accent: "#38bdf8",
    highlight: "#818cf8",
    blurPx: 22,
    glassOpacity: 0.76,
    borderRefraction: 78,
    dynamicLighting: true,
    cornerRadius: 16,
  },
  {
    id: "cyberpunk-neon",
    name: "Cyberpunk Neon",
    surfaceMode: "liquid-glass",
    background: "#090412",
    surface: "#160a26",
    text: "#f0f9ff",
    mutedText: "#b89fd6",
    accent: "#00f0ff",
    highlight: "#ff00aa",
    blurPx: 18,
    glassOpacity: 0.78,
    borderRefraction: 92,
    dynamicLighting: true,
    cornerRadius: 10,
  },
  {
    id: "obsidian-dark",
    name: "Obsidian Dark",
    surfaceMode: "solid-minimal",
    background: "#09090b",
    surface: "#141418",
    text: "#f4f4f5",
    mutedText: "#a1a1aa",
    accent: "#e4e4e7",
    highlight: "#71717a",
    blurPx: 0,
    glassOpacity: 1,
    borderRefraction: 20,
    dynamicLighting: false,
    cornerRadius: 12,
  },
  {
    id: "arctic-frost",
    name: "Arctic Frost",
    surfaceMode: "liquid-glass",
    background: "#e6f2fc",
    surface: "#f8fbff",
    text: "#0b1d33",
    mutedText: "#4a6482",
    accent: "#0284c7",
    highlight: "#38bdf8",
    blurPx: 26,
    glassOpacity: 0.8,
    borderRefraction: 88,
    dynamicLighting: true,
    cornerRadius: 18,
  },
  {
    id: "solar-gold",
    name: "Solar Gold",
    surfaceMode: "liquid-glass",
    background: "#120d06",
    surface: "#1f170b",
    text: "#fef7e0",
    mutedText: "#c9b07a",
    accent: "#f59e0b",
    highlight: "#fde047",
    blurPx: 18,
    glassOpacity: 0.8,
    borderRefraction: 76,
    dynamicLighting: true,
    cornerRadius: 14,
  },
  {
    id: "emerald-matrix",
    name: "Emerald Matrix",
    surfaceMode: "solid-minimal",
    background: "#040b07",
    surface: "#0a1710",
    text: "#dcfce7",
    mutedText: "#6ee7b7",
    accent: "#10b981",
    highlight: "#34d399",
    blurPx: 0,
    glassOpacity: 1,
    borderRefraction: 30,
    dynamicLighting: false,
    cornerRadius: 8,
  },
  {
    id: "crimson-pulse",
    name: "Crimson Pulse",
    surfaceMode: "liquid-glass",
    background: "#130508",
    surface: "#210a10",
    text: "#fff1f2",
    mutedText: "#fda4af",
    accent: "#f43f5e",
    highlight: "#fb923c",
    blurPx: 20,
    glassOpacity: 0.78,
    borderRefraction: 82,
    dynamicLighting: true,
    cornerRadius: 14,
  },
];

export const DEFAULT_THEME_CONFIG: ThemeConfig = { ...THEME_PRESETS[0] };

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const cleaned = hex.trim().replace(/^#/, "");
  const full =
    cleaned.length === 3
      ? cleaned
          .split("")
          .map((c) => c + c)
          .join("")
      : cleaned;

  if (!/^[0-9a-fA-F]{6}$/.test(full)) {
    return { r: 56, g: 189, b: 248 };
  }

  return {
    r: Number.parseInt(full.slice(0, 2), 16),
    g: Number.parseInt(full.slice(2, 4), 16),
    b: Number.parseInt(full.slice(4, 6), 16),
  };
}

export function isLightColor(hex: string): boolean {
  const { r, g, b } = hexToRgb(hex);
  const toLinear = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  const lum = 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
  return lum > 0.42;
}

export function getContrastForeground(hex: string): "#05070c" | "#ffffff" {
  return isLightColor(hex) ? "#05070c" : "#ffffff";
}

export function applyThemeToDocument(theme: ThemeConfig): void {
  const root = document.documentElement;
  const bgRgb = hexToRgb(theme.background);
  const panelRgb = hexToRgb(theme.surface);
  const accentRgb = hexToRgb(theme.accent);
  const highlightRgb = hexToRgb(theme.highlight);
  const lightSurface = isLightColor(theme.background);
  const accentFg = getContrastForeground(theme.accent);
  const isGlass = theme.surfaceMode === "liquid-glass";

  root.style.setProperty("--bg-color", theme.background);
  root.style.setProperty("--panel-color", theme.surface);
  root.style.setProperty("--text-color", theme.text);
  root.style.setProperty("--muted-text", theme.mutedText);
  root.style.setProperty("--accent", theme.accent);
  root.style.setProperty("--accent-fg", accentFg);
  root.style.setProperty("--accent-glow", `rgba(${accentRgb.r}, ${accentRgb.g}, ${accentRgb.b}, 0.42)`);
  root.style.setProperty("--highlight", theme.highlight);
  root.style.setProperty("--highlight-glow", `rgba(${highlightRgb.r}, ${highlightRgb.g}, ${highlightRgb.b}, 0.36)`);

  root.style.setProperty("--bg-rgb", `${bgRgb.r}, ${bgRgb.g}, ${bgRgb.b}`);
  root.style.setProperty("--panel-rgb", `${panelRgb.r}, ${panelRgb.g}, ${panelRgb.b}`);
  root.style.setProperty("--accent-rgb", `${accentRgb.r}, ${accentRgb.g}, ${accentRgb.b}`);
  root.style.setProperty("--highlight-rgb", `${highlightRgb.r}, ${highlightRgb.g}, ${highlightRgb.b}`);

  root.style.setProperty("--glass-blur", isGlass ? `${theme.blurPx}px` : "0px");
  root.style.setProperty(
    "--glass-opacity",
    isGlass ? String(Math.min(1, Math.max(0.35, theme.glassOpacity))) : "1",
  );
  root.style.setProperty(
    "--panel-opacity",
    isGlass ? String(Math.min(0.95, Math.max(0.28, theme.glassOpacity - 0.14))) : "1",
  );
  root.style.setProperty(
    "--refraction-opacity",
    String(Math.min(1, Math.max(0, theme.borderRefraction / 100))),
  );
  root.style.setProperty("--radius", `${theme.cornerRadius}px`);

  root.style.setProperty(
    "--surface-subtle",
    lightSurface ? "rgba(15, 23, 42, 0.06)" : "rgba(255, 255, 255, 0.05)",
  );
  root.style.setProperty(
    "--surface-input",
    lightSurface ? "rgba(255, 255, 255, 0.72)" : "rgba(0, 0, 0, 0.36)",
  );
  root.style.setProperty(
    "--border-subtle",
    lightSurface ? "rgba(15, 23, 42, 0.14)" : "rgba(255, 255, 255, 0.10)",
  );
}
