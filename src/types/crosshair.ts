import { DEFAULT_THEME_CONFIG, type ThemeConfig } from "./themes";

export type CrosshairDotShape = "circle" | "square";
export type CrosshairColorMode = "solid" | "gradient" | "rainbow";

export interface CrosshairConfig {
  color: string;
  outline: boolean;
  outlineColor: string;
  outlineThickness: number;
  length: number;
  thickness: number;
  gap: number;
  rotation: number;
  dot: boolean;
  dotSize: number;
  dotShape: CrosshairDotShape;
  tShape: boolean;
  opacity: number;
  outlineOpacity: number;
  blur: number;
  offsetX: number;
  offsetY: number;
  colorMode: CrosshairColorMode;
  gradientColor: string;
  gradientAngle: number;
  dynamicSpread: boolean;
  spreadAmount: number;
  spreadSpeed: number;
  recoilColorEnabled: boolean;
  recoilColor: string;
  animate: boolean;
  spinSpeed: number;
  rainbow: boolean;
  rainbowSpeed: number;
  imgSrc: string | null;
  hasCircle: boolean;
  circleRadius: number;
  hideBars: boolean;
  hideCrosshair: boolean;
  showTop: boolean;
  showBottom: boolean;
  showLeft: boolean;
  showRight: boolean;
}

export interface NumericRange {
  min: number;
  max: number;
  step: number;
}

export const CROSSHAIR_RANGES: Record<
  | "outlineThickness"
  | "length"
  | "thickness"
  | "gap"
  | "rotation"
  | "dotSize"
  | "opacity"
  | "outlineOpacity"
  | "blur"
  | "offsetX"
  | "offsetY"
  | "gradientAngle"
  | "spreadAmount"
  | "spreadSpeed"
  | "circleRadius"
  | "spinSpeed"
  | "rainbowSpeed",
  NumericRange
> = {
  outlineThickness: { min: 0, max: 8, step: 1 },
  length: { min: 0, max: 60, step: 1 },
  thickness: { min: 1, max: 24, step: 1 },
  gap: { min: 0, max: 60, step: 1 },
  rotation: { min: 0, max: 360, step: 1 },
  dotSize: { min: 1, max: 20, step: 1 },
  opacity: { min: 0.1, max: 1, step: 0.05 },
  outlineOpacity: { min: 0, max: 1, step: 0.05 },
  blur: { min: 0, max: 20, step: 1 },
  offsetX: { min: -100, max: 100, step: 1 },
  offsetY: { min: -100, max: 100, step: 1 },
  gradientAngle: { min: 0, max: 360, step: 1 },
  spreadAmount: { min: 2, max: 60, step: 1 },
  spreadSpeed: { min: 1, max: 30, step: 1 },
  circleRadius: { min: 1, max: 200, step: 1 },
  spinSpeed: { min: 1, max: 20, step: 1 },
  rainbowSpeed: { min: 1, max: 20, step: 1 },
};

export type UiWindowStyle = "glass" | "solid" | "neon";
export type UiButtonStyle = "modern" | "classic" | "pill";
export type UiLayoutMode = "split" | "og-right" | "og-left";

export interface UiConfig {
  uiTheme: string;
  uiWindow: UiWindowStyle;
  uiButton: UiButtonStyle;
  uiLayout: UiLayoutMode;
  uiAccent: string;
  theme: ThemeConfig;
  hotkey: string;
  adsHotkey: string | null;
  enableProfileHotkeys: boolean;
  streamproof: boolean;
  cloakMenu: boolean;
}

export interface CrosshairProfile {
  id: string;
  name: string;
  hotkey: string | null;
  config: CrosshairConfig;
}

export interface LuairState {
  crosshair: CrosshairConfig;
  ui: UiConfig;
  profiles: CrosshairProfile[];
  activeProfileId: string | null;
}

export interface CloakStatus {
  enabled: boolean;
  cloakMenu: boolean;
  platform: string;
  mechanism: string;
  supported: boolean;
  overlayVerified: boolean;
}

export const DEFAULT_CROSSHAIR_CONFIG: CrosshairConfig = {
  color: "#06b6d4",
  outline: true,
  outlineColor: "#000000",
  outlineThickness: 1,
  length: 10,
  thickness: 2,
  gap: 5,
  rotation: 0,
  dot: false,
  dotSize: 3,
  dotShape: "circle",
  tShape: false,
  opacity: 1,
  outlineOpacity: 1,
  blur: 0,
  offsetX: 0,
  offsetY: 0,
  colorMode: "solid",
  gradientColor: "#818cf8",
  gradientAngle: 45,
  dynamicSpread: false,
  spreadAmount: 14,
  spreadSpeed: 10,
  recoilColorEnabled: false,
  recoilColor: "#ff2a55",
  animate: false,
  spinSpeed: 2,
  rainbow: false,
  rainbowSpeed: 5,
  imgSrc: null,
  hasCircle: false,
  circleRadius: 15,
  hideBars: false,
  hideCrosshair: false,
  showTop: true,
  showBottom: true,
  showLeft: true,
  showRight: true,
};

export const DEFAULT_UI_CONFIG: UiConfig = {
  uiTheme: "Deep Space",
  uiWindow: "glass",
  uiButton: "modern",
  uiLayout: "split",
  uiAccent: DEFAULT_THEME_CONFIG.accent,
  theme: { ...DEFAULT_THEME_CONFIG },
  hotkey: "F10",
  adsHotkey: "Alt+X",
  enableProfileHotkeys: true,
  streamproof: false,
  cloakMenu: true,
};

export const DEFAULT_PROFILES: CrosshairProfile[] = [
  {
    id: "slot-1",
    name: "Primary (Rifle)",
    hotkey: "1",
    config: { ...DEFAULT_CROSSHAIR_CONFIG },
  },
  {
    id: "slot-2",
    name: "Secondary (Precision Dot)",
    hotkey: "2",
    config: {
      ...DEFAULT_CROSSHAIR_CONFIG,
      color: "#22c55e",
      hideBars: true,
      dot: true,
      dotSize: 5,
      dotShape: "circle",
      thickness: 4,
      outline: true,
    },
  },
  {
    id: "slot-3",
    name: "Close Quarters (Dynamic)",
    hotkey: "3",
    config: {
      ...DEFAULT_CROSSHAIR_CONFIG,
      color: "#f59e0b",
      hideBars: true,
      dot: true,
      dotSize: 3,
      hasCircle: true,
      circleRadius: 24,
      thickness: 2,
      outline: true,
      dynamicSpread: true,
      spreadAmount: 16,
      recoilColorEnabled: true,
      recoilColor: "#ff2a55",
    },
  },
  {
    id: "slot-4",
    name: "Tactical T-Shape",
    hotkey: "Mouse4",
    config: {
      ...DEFAULT_CROSSHAIR_CONFIG,
      color: "#22d3ee",
      colorMode: "gradient",
      gradientColor: "#818cf8",
      gradientAngle: 45,
      tShape: true,
      dot: true,
      dotSize: 3,
      length: 12,
      gap: 4,
    },
  },
];

export const DEFAULT_LUAIR_STATE: LuairState = {
  crosshair: { ...DEFAULT_CROSSHAIR_CONFIG },
  ui: { ...DEFAULT_UI_CONFIG, theme: { ...DEFAULT_THEME_CONFIG } },
  profiles: DEFAULT_PROFILES.map((p) => ({ ...p, config: { ...p.config } })),
  activeProfileId: "slot-1",
};

function clampRange(value: unknown, fallback: number, range: NumericRange): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  const clamped = Math.min(range.max, Math.max(range.min, value));
  if (range.step < 1) {
    return Math.round(clamped * 100) / 100;
  }
  return Math.round(clamped);
}

function readBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function readColorString(value: unknown, fallback: string): string {
  if (typeof value === "string" && value.trim().length > 0) {
    return value.trim();
  }
  return fallback;
}

function readDotShape(value: unknown, fallback: CrosshairDotShape): CrosshairDotShape {
  return value === "circle" || value === "square" ? value : fallback;
}

function readColorMode(
  value: unknown,
  legacyRainbow: boolean,
  fallback: CrosshairColorMode,
): CrosshairColorMode {
  if (value === "solid" || value === "gradient" || value === "rainbow") {
    return value;
  }
  return legacyRainbow ? "rainbow" : fallback;
}

/**
 * Validates and normalizes either a flat legacy `.luair`/`.json` crosshair payload
 * or a nested `{ crosshair }` / `{ data }` object while dropping unknown keys.
 */
export function parseCrosshairConfig(raw: unknown): CrosshairConfig {
  if (typeof raw !== "object" || raw === null) {
    return { ...DEFAULT_CROSSHAIR_CONFIG };
  }

  const record = raw as Record<string, unknown>;
  const source =
    typeof record.crosshair === "object" && record.crosshair !== null
      ? (record.crosshair as Record<string, unknown>)
      : typeof record.data === "object" && record.data !== null
        ? (record.data as Record<string, unknown>)
        : record;

  const legacyRainbow = readBoolean(
    source.rainbow,
    DEFAULT_CROSSHAIR_CONFIG.rainbow,
  );
  const colorMode = readColorMode(
    source.colorMode,
    legacyRainbow,
    DEFAULT_CROSSHAIR_CONFIG.colorMode,
  );

  return {
    color: readColorString(source.color, DEFAULT_CROSSHAIR_CONFIG.color),
    outline: readBoolean(source.outline, DEFAULT_CROSSHAIR_CONFIG.outline),
    outlineColor: readColorString(
      source.outlineColor,
      DEFAULT_CROSSHAIR_CONFIG.outlineColor,
    ),
    outlineThickness: clampRange(
      source.outlineThickness,
      DEFAULT_CROSSHAIR_CONFIG.outlineThickness,
      CROSSHAIR_RANGES.outlineThickness,
    ),
    length: clampRange(
      source.length,
      DEFAULT_CROSSHAIR_CONFIG.length,
      CROSSHAIR_RANGES.length,
    ),
    thickness: clampRange(
      source.thickness,
      DEFAULT_CROSSHAIR_CONFIG.thickness,
      CROSSHAIR_RANGES.thickness,
    ),
    gap: clampRange(
      source.gap,
      DEFAULT_CROSSHAIR_CONFIG.gap,
      CROSSHAIR_RANGES.gap,
    ),
    rotation: clampRange(
      source.rotation,
      DEFAULT_CROSSHAIR_CONFIG.rotation,
      CROSSHAIR_RANGES.rotation,
    ),
    dot: readBoolean(source.dot, DEFAULT_CROSSHAIR_CONFIG.dot),
    dotSize: clampRange(
      source.dotSize,
      DEFAULT_CROSSHAIR_CONFIG.dotSize,
      CROSSHAIR_RANGES.dotSize,
    ),
    dotShape: readDotShape(source.dotShape, DEFAULT_CROSSHAIR_CONFIG.dotShape),
    tShape: readBoolean(source.tShape, DEFAULT_CROSSHAIR_CONFIG.tShape),
    opacity: clampRange(
      source.opacity,
      DEFAULT_CROSSHAIR_CONFIG.opacity,
      CROSSHAIR_RANGES.opacity,
    ),
    outlineOpacity: clampRange(
      source.outlineOpacity,
      DEFAULT_CROSSHAIR_CONFIG.outlineOpacity,
      CROSSHAIR_RANGES.outlineOpacity,
    ),
    blur: clampRange(
      source.blur,
      DEFAULT_CROSSHAIR_CONFIG.blur,
      CROSSHAIR_RANGES.blur,
    ),
    offsetX: clampRange(
      source.offsetX,
      DEFAULT_CROSSHAIR_CONFIG.offsetX,
      CROSSHAIR_RANGES.offsetX,
    ),
    offsetY: clampRange(
      source.offsetY,
      DEFAULT_CROSSHAIR_CONFIG.offsetY,
      CROSSHAIR_RANGES.offsetY,
    ),
    colorMode,
    gradientColor: readColorString(
      source.gradientColor,
      DEFAULT_CROSSHAIR_CONFIG.gradientColor,
    ),
    gradientAngle: clampRange(
      source.gradientAngle,
      DEFAULT_CROSSHAIR_CONFIG.gradientAngle,
      CROSSHAIR_RANGES.gradientAngle,
    ),
    dynamicSpread: readBoolean(
      source.dynamicSpread,
      DEFAULT_CROSSHAIR_CONFIG.dynamicSpread,
    ),
    spreadAmount: clampRange(
      source.spreadAmount,
      DEFAULT_CROSSHAIR_CONFIG.spreadAmount,
      CROSSHAIR_RANGES.spreadAmount,
    ),
    spreadSpeed: clampRange(
      source.spreadSpeed,
      DEFAULT_CROSSHAIR_CONFIG.spreadSpeed,
      CROSSHAIR_RANGES.spreadSpeed,
    ),
    recoilColorEnabled: readBoolean(
      source.recoilColorEnabled,
      DEFAULT_CROSSHAIR_CONFIG.recoilColorEnabled,
    ),
    recoilColor: readColorString(
      source.recoilColor,
      DEFAULT_CROSSHAIR_CONFIG.recoilColor,
    ),
    animate: readBoolean(source.animate, DEFAULT_CROSSHAIR_CONFIG.animate),
    spinSpeed: clampRange(
      source.spinSpeed,
      DEFAULT_CROSSHAIR_CONFIG.spinSpeed,
      CROSSHAIR_RANGES.spinSpeed,
    ),
    rainbow: colorMode === "rainbow",
    rainbowSpeed: clampRange(
      source.rainbowSpeed,
      DEFAULT_CROSSHAIR_CONFIG.rainbowSpeed,
      CROSSHAIR_RANGES.rainbowSpeed,
    ),
    imgSrc:
      typeof source.imgSrc === "string" && source.imgSrc.length > 0
        ? source.imgSrc
        : null,
    hasCircle: readBoolean(source.hasCircle, DEFAULT_CROSSHAIR_CONFIG.hasCircle),
    circleRadius: clampRange(
      source.circleRadius,
      DEFAULT_CROSSHAIR_CONFIG.circleRadius,
      CROSSHAIR_RANGES.circleRadius,
    ),
    hideBars: readBoolean(source.hideBars, DEFAULT_CROSSHAIR_CONFIG.hideBars),
    hideCrosshair: readBoolean(
      source.hideCrosshair,
      DEFAULT_CROSSHAIR_CONFIG.hideCrosshair,
    ),
    showTop: readBoolean(source.showTop, DEFAULT_CROSSHAIR_CONFIG.showTop),
    showBottom: readBoolean(
      source.showBottom,
      DEFAULT_CROSSHAIR_CONFIG.showBottom,
    ),
    showLeft: readBoolean(source.showLeft, DEFAULT_CROSSHAIR_CONFIG.showLeft),
    showRight: readBoolean(source.showRight, DEFAULT_CROSSHAIR_CONFIG.showRight),
  };
}
