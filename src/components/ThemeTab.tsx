import { useRef, useState, type ChangeEvent } from "react";
import { motion } from "framer-motion";
import {
  Check,
  Copy,
  Download,
  FileUp,
  Palette,
  Sparkles,
  Square,
  Upload,
} from "lucide-react";
import {
  encodeThemeCode,
  exportThemeJson,
  importThemeInput,
} from "../lib/themeCodec";
import {
  isLightColor,
  THEME_PRESETS,
  type ThemeConfig,
  type ThemeSurfaceMode,
} from "../types/themes";
import type { LuairState } from "../types/crosshair";
import { MakeWithAiBar } from "./MakeWithAiBar";
import { PanelCard, SliderRow, ToggleChip } from "./UiPrimitives";

interface ThemeTabProps {
  theme: ThemeConfig;
  onChangeTheme: (next: ThemeConfig, immediate?: boolean) => void;
  onAiApplied: (nextState: LuairState) => void;
}

interface ColorFieldProps {
  label: string;
  value: string;
  onChange: (hex: string) => void;
}

function ColorField({ label, value, onChange }: ColorFieldProps) {
  return (
    <label className="space-y-1">
      <span className="block text-[10px] font-medium text-[var(--muted-text)]">
        {label}
      </span>
      <div
        style={{
          backgroundColor: "var(--surface-input)",
          borderColor: "var(--border-subtle)",
        }}
        className="flex items-center gap-2 rounded-lg border px-2 py-1"
      >
        <input
          type="color"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="h-6 w-7 shrink-0 cursor-pointer rounded border-0 bg-transparent"
        />
        <input
          type="text"
          value={value}
          onChange={(e) => {
            const val = e.target.value.trim();
            if (/^#[0-9a-fA-F]{6}$/.test(val)) {
              onChange(val.toLowerCase());
            }
          }}
          aria-label={`${label} hex`}
          className="w-full min-w-0 bg-transparent font-mono text-[11px] uppercase outline-none"
        />
      </div>
    </label>
  );
}

export function ThemeTab({ theme, onChangeTheme, onAiApplied }: ThemeTabProps) {
  const [importText, setImportText] = useState("");
  const [copiedMode, setCopiedMode] = useState<"code" | "json" | null>(null);
  const [feedback, setFeedback] = useState<{
    kind: "ok" | "err";
    msg: string;
  } | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const shareCode = encodeThemeCode(theme);

  const updateField = <K extends keyof ThemeConfig>(
    key: K,
    value: ThemeConfig[K],
    immediate = false,
  ) => {
    onChangeTheme(
      {
        ...theme,
        id: "custom",
        name: "Custom",
        [key]: value,
      },
      immediate,
    );
  };

  const handleSurfaceMode = (mode: ThemeSurfaceMode) => {
    if (mode === theme.surfaceMode) return;
    onChangeTheme(
      {
        ...theme,
        surfaceMode: mode,
        blurPx: mode === "liquid-glass" && theme.blurPx === 0 ? 20 : theme.blurPx,
        glassOpacity:
          mode === "liquid-glass" && theme.glassOpacity === 1 ? 0.78 : theme.glassOpacity,
      },
      true,
    );
  };

  const handleCopy = async (kind: "code" | "json") => {
    const payload = kind === "code" ? shareCode : exportThemeJson(theme);
    try {
      await navigator.clipboard.writeText(payload);
      setCopiedMode(kind);
      window.setTimeout(() => {
        setCopiedMode((cur) => (cur === kind ? null : cur));
      }, 1800);
    } catch {
      setImportText(payload);
      setFeedback({
        kind: "ok",
        msg: "Clipboard unavailable — placed exported theme into the box below.",
      });
    }
  };

  const handleDownloadJson = () => {
    const json = exportThemeJson(theme);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${theme.id || "luair-theme"}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const handleImportSubmit = (raw = importText) => {
    const result = importThemeInput(raw);
    if (!result.ok) {
      setFeedback({ kind: "err", msg: result.error });
      return;
    }
    onChangeTheme(result.theme, true);
    setImportText("");
    setFeedback({
      kind: "ok",
      msg: `Applied "${result.theme.name}" from ${result.format === "code" ? "share code" : "JSON"}.`,
    });
  };

  const handleFileLoad = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const text = ev.target?.result;
      if (typeof text === "string") {
        handleImportSubmit(text);
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  };

  return (
    <div className="space-y-4">
      <MakeWithAiBar
        mode="theme"
        examples={[
          "Make a neon purple liquid glass theme",
          "Matte obsidian crimson solid minimalist theme",
          "Arctic frost liquid glass with heavy blur",
        ]}
        onApplied={onAiApplied}
      />

      {/* Surface Architecture Toggle */}
      <PanelCard
        surfaceMode={theme.surfaceMode}
        className="flex flex-wrap items-center justify-between gap-3 p-4"
      >
        <div>
          <h2 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Surface Material Engine
          </h2>
          <p className="mt-0.5 text-[11px] text-[var(--muted-text)]">
            Switch seamlessly between refractive Liquid Glass and Solid Modern Minimalist.
          </p>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="relative grid grid-cols-2 rounded-xl border p-1"
        >
          {(
            [
              { id: "liquid-glass", label: "Liquid Glass", Icon: Sparkles },
              { id: "solid-minimal", label: "Solid Minimalist", Icon: Square },
            ] as const
          ).map(({ id, label, Icon }) => {
            const active = theme.surfaceMode === id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => handleSurfaceMode(id)}
                style={{
                  color: active ? "var(--accent-fg)" : "var(--text-color)",
                }}
                className="relative z-10 flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[11px] font-bold transition-colors"
              >
                {active && (
                  <motion.span
                    layoutId="surface-mode-pill"
                    transition={{ type: "spring", stiffness: 420, damping: 32 }}
                    className="absolute inset-0 -z-10 rounded-lg bg-[var(--accent)] shadow-[0_0_16px_var(--accent-glow)]"
                  />
                )}
                <Icon className="h-3.5 w-3.5" />
                {label}
              </button>
            );
          })}
        </div>
      </PanelCard>

      {/* Curated Presets Grid */}
      <PanelCard surfaceMode={theme.surfaceMode} className="space-y-3 p-4">
        <div className="flex items-center justify-between">
          <h3 className="flex items-center gap-1.5 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            <Palette className="h-3.5 w-3.5" />
            Built-in Presets ({THEME_PRESETS.length})
          </h3>
          <span className="text-[10px] text-[var(--muted-text)]">
            Active: <strong className="text-[var(--text-color)]">{theme.name}</strong>
          </span>
        </div>

        <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-4">
          {THEME_PRESETS.map((preset) => {
            const selected = theme.id === preset.id;
            return (
              <motion.button
                key={preset.id}
                type="button"
                whileHover={{ y: -2 }}
                whileTap={{ scale: 0.98 }}
                onClick={() => onChangeTheme({ ...preset }, true)}
                style={{
                  backgroundColor: preset.background,
                  borderColor: selected ? preset.accent : "var(--border-subtle)",
                  color: preset.text,
                  boxShadow: selected ? `0 0 18px ${preset.accent}44` : undefined,
                }}
                className="group relative flex flex-col justify-between overflow-hidden rounded-xl border p-2.5 text-left transition"
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-1">
                    <span
                      className="h-3 w-3 rounded-full border border-white/25"
                      style={{ backgroundColor: preset.accent }}
                    />
                    <span
                      className="h-3 w-3 rounded-full border border-white/25"
                      style={{ backgroundColor: preset.highlight }}
                    />
                    <span
                      className="h-3 w-3 rounded-full border border-white/25"
                      style={{ backgroundColor: preset.surface }}
                    />
                  </div>
                  <span
                    className="rounded px-1.5 py-0.5 text-[9px] font-bold uppercase"
                    style={{
                      backgroundColor: preset.surface,
                      color: preset.mutedText,
                    }}
                  >
                    {preset.surfaceMode === "liquid-glass" ? "Glass" : "Solid"}
                  </span>
                </div>
                <div className="mt-3 flex items-center justify-between">
                  <span className="truncate text-[11px] font-bold">{preset.name}</span>
                  {selected && (
                    <Check className="h-3.5 w-3.5 shrink-0" style={{ color: preset.accent }} />
                  )}
                </div>
              </motion.button>
            );
          })}
        </div>
      </PanelCard>

      {/* Color Pickers + Material & Lighting Controls */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <PanelCard surfaceMode={theme.surfaceMode} className="space-y-3 p-4">
          <div className="flex items-center justify-between">
            <h3 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
              Color Palette
            </h3>
            <button
              type="button"
              onClick={() => {
                const light = isLightColor(theme.background);
                onChangeTheme(
                  {
                    ...theme,
                    id: "custom",
                    name: "Custom",
                    text: light ? "#0b1d33" : "#eef4ff",
                    mutedText: light ? "#4a6482" : "#8a99bd",
                  },
                  true,
                );
              }}
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="rounded-md border px-2 py-0.5 text-[10px] font-semibold transition hover:border-[var(--accent)]"
            >
              Auto Contrast Text
            </button>
          </div>

          <div className="grid grid-cols-2 gap-2.5">
            <ColorField
              label="Accent"
              value={theme.accent}
              onChange={(v) => updateField("accent", v)}
            />
            <ColorField
              label="Highlight"
              value={theme.highlight}
              onChange={(v) => updateField("highlight", v)}
            />
            <ColorField
              label="Background"
              value={theme.background}
              onChange={(v) => updateField("background", v)}
            />
            <ColorField
              label="Panel Surface"
              value={theme.surface}
              onChange={(v) => updateField("surface", v)}
            />
            <ColorField
              label="Primary Text"
              value={theme.text}
              onChange={(v) => updateField("text", v)}
            />
            <ColorField
              label="Muted Text"
              value={theme.mutedText}
              onChange={(v) => updateField("mutedText", v)}
            />
          </div>
        </PanelCard>

        <PanelCard surfaceMode={theme.surfaceMode} className="space-y-3 p-4">
          <h3 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Refraction, Blur & Geometry
          </h3>

          <SliderRow
            label="Corner Radius"
            value={theme.cornerRadius}
            unit="px"
            min={0}
            max={24}
            step={1}
            onChange={(v) => updateField("cornerRadius", v)}
          />

          {theme.surfaceMode === "liquid-glass" ? (
            <>
              <SliderRow
                label="Backdrop Blur"
                value={theme.blurPx}
                unit="px"
                min={0}
                max={36}
                step={1}
                onChange={(v) => updateField("blurPx", v)}
              />
              <SliderRow
                label="Glass Opacity"
                value={Math.round(theme.glassOpacity * 100)}
                unit="%"
                min={35}
                max={100}
                step={1}
                onChange={(v) => updateField("glassOpacity", Number((v / 100).toFixed(2)))}
              />
              <SliderRow
                label="Border Refraction"
                value={theme.borderRefraction}
                unit="%"
                min={0}
                max={100}
                step={1}
                onChange={(v) => updateField("borderRefraction", v)}
              />
              <ToggleChip
                label="Cursor-Reactive Dynamic Lighting"
                checked={theme.dynamicLighting}
                onChange={(v) => updateField("dynamicLighting", v, true)}
              />
            </>
          ) : (
            <div
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="rounded-xl border p-3 text-[11px] text-[var(--muted-text)]"
            >
              Solid Minimalist mode disables backdrop blur and refractive shaders for crisp,
              high-contrast matte panels. Switch to <strong>Liquid Glass</strong> above to
              tune blur, translucency, and cursor lighting.
            </div>
          )}
        </PanelCard>
      </div>

      {/* Shareable Theme Export / Import */}
      <PanelCard surfaceMode={theme.surfaceMode} className="space-y-3 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
              Theme Export & Import
            </h3>
            <p className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              Share themes as compact <code className="font-mono">LUAIR1:</code> codes or
              clean JSON files.
            </p>
          </div>

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void handleCopy("code")}
              className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-[11px] font-bold text-[var(--accent-fg)] transition hover:opacity-90"
            >
              {copiedMode === "code" ? (
                <Check className="h-3.5 w-3.5" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
              {copiedMode === "code" ? "Code Copied!" : "Copy Share Code"}
            </button>
            <button
              type="button"
              onClick={() => void handleCopy("json")}
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
            >
              {copiedMode === "json" ? (
                <Check className="h-3.5 w-3.5 text-emerald-400" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
              {copiedMode === "json" ? "JSON Copied!" : "Copy JSON"}
            </button>
            <button
              type="button"
              onClick={handleDownloadJson}
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
            >
              <Download className="h-3.5 w-3.5" />
              Save .json
            </button>
          </div>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="truncate rounded-lg border px-3 py-1.5 font-mono text-[10px] text-[var(--muted-text)]"
          title={shareCode}
        >
          {shareCode}
        </div>

        <div className="space-y-2">
          <textarea
            rows={2}
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder="Paste LUAIR1:... share code or theme JSON here..."
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
              color: "var(--text-color)",
            }}
            className="luair-scroll w-full resize-none rounded-xl border p-2.5 font-mono text-[11px] outline-none focus:border-[var(--accent)]"
          />
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => handleImportSubmit()}
                className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-[11px] font-bold text-[var(--accent-fg)] transition hover:opacity-90"
              >
                <Upload className="h-3.5 w-3.5" />
                Import & Apply
              </button>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json,.txt"
                onChange={handleFileLoad}
                className="hidden"
              />
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
              >
                <FileUp className="h-3.5 w-3.5" />
                Load .json File
              </button>
            </div>

            {feedback && (
              <span
                className={`text-[11px] font-semibold ${
                  feedback.kind === "ok" ? "text-emerald-400" : "text-rose-400"
                }`}
              >
                {feedback.msg}
              </span>
            )}
          </div>
        </div>
      </PanelCard>
    </div>
  );
}
