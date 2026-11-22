import { useRef, useState, type ChangeEvent } from "react";
import {
  Check,
  Copy,
  Crosshair,
  Eye,
  EyeOff,
  Flame,
  RotateCcw,
  Upload,
  X,
} from "lucide-react";
import { CrosshairCanvas } from "./CrosshairCanvas";
import { MakeWithAiBar } from "./MakeWithAiBar";
import { PanelCard, SliderRow, ToggleChip } from "./UiPrimitives";
import type { SpreadSimMode } from "../engine/crosshairRenderer";
import { encodeCrosshairCode } from "../lib/crosshairCodec";
import {
  CROSSHAIR_RANGES,
  DEFAULT_CROSSHAIR_CONFIG,
  type CrosshairColorMode,
  type CrosshairConfig,
  type CrosshairDotShape,
  type LuairState,
} from "../types/crosshair";
import type { ThemeSurfaceMode } from "../types/themes";

type PreviewBackdrop = "dark" | "slate" | "bright";

interface CrosshairTabProps {
  crosshair: CrosshairConfig;
  activeProfileId: string | null;
  surfaceMode: ThemeSurfaceMode;
  onUpdate: <K extends keyof CrosshairConfig>(
    key: K,
    value: CrosshairConfig[K],
  ) => void;
  onReset: () => void;
  onAiApplied: (nextState: LuairState) => void;
}

interface ColorPickerFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
}

function ColorPickerField({ label, value, onChange }: ColorPickerFieldProps) {
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
          className="h-6 w-8 shrink-0 cursor-pointer rounded border-0 bg-transparent"
        />
        <span className="font-mono text-[11px] uppercase">{value}</span>
      </div>
    </label>
  );
}

export function CrosshairTab({
  crosshair,
  activeProfileId,
  surfaceMode,
  onUpdate,
  onReset,
  onAiApplied,
}: CrosshairTabProps) {
  const [backdrop, setBackdrop] = useState<PreviewBackdrop>("dark");
  const [spreadSim, setSpreadSim] = useState<SpreadSimMode>("off");
  const [holdFiring, setHoldFiring] = useState(false);
  const [copiedCode, setCopiedCode] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const handleImageUpload = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const result = ev.target?.result;
      if (typeof result === "string") {
        onUpdate("imgSrc", result);
      }
    };
    reader.readAsDataURL(file);
  };

  const handleCopyCode = async () => {
    const code = encodeCrosshairCode(crosshair);
    try {
      await navigator.clipboard.writeText(code);
      setCopiedCode(true);
      window.setTimeout(() => setCopiedCode(false), 1600);
    } catch {
      // Clipboard blocked outside secure/desktop context.
    }
  };

  const backdropStyle =
    backdrop === "bright"
      ? "bg-gradient-to-br from-sky-200 via-slate-100 to-indigo-200 text-slate-900"
      : backdrop === "slate"
        ? "bg-gradient-to-br from-slate-700 via-slate-800 to-zinc-800 text-white"
        : "bg-[radial-gradient(circle_at_center,rgba(255,255,255,0.08)_1px,transparent_1px)] bg-[#06080f] [background-size:18px_18px] text-white";

  const simModes: { id: SpreadSimMode; label: string }[] = [
    { id: "off", label: "Off" },
    { id: "tap", label: "Tap" },
    { id: "burst", label: "Burst" },
    { id: "spray", label: "Spray" },
  ];

  return (
    <div className="space-y-4">
      <MakeWithAiBar
        mode="crosshair"
        targetSlotId={activeProfileId}
        examples={[
          "Make a clean lime green CS2 dot with a thin border and bind it to key 2",
          "Cyan gradient T-shape with dynamic firing spread",
          "Small crimson precision dot with soft glow",
        ]}
        onApplied={onAiApplied}
      />

      {/* Live Interactive Studio Stage */}
      <PanelCard
        surfaceMode={surfaceMode}
        className={`relative flex h-48 flex-col justify-between overflow-hidden ${backdropStyle}`}
      >
        <div
          onPointerDown={() => setHoldFiring(true)}
          onPointerUp={() => setHoldFiring(false)}
          onPointerLeave={() => setHoldFiring(false)}
          onPointerCancel={() => setHoldFiring(false)}
          className="absolute inset-0 z-10 cursor-crosshair"
          title="Click & hold to test firing spread and recoil color"
        />

        <div className="relative z-20 flex flex-wrap items-center justify-between gap-2 p-2.5">
          <div className="flex items-center gap-1.5">
            <span className="rounded-md bg-black/45 px-2 py-0.5 text-[10px] font-bold tracking-wider text-white uppercase backdrop-blur-sm">
              Studio Stage
            </span>
            {(["dark", "slate", "bright"] as PreviewBackdrop[]).map((mode) => (
              <button
                key={mode}
                type="button"
                onClick={() => setBackdrop(mode)}
                className={`rounded-md px-2 py-0.5 text-[10px] font-semibold capitalize transition ${
                  backdrop === mode
                    ? "bg-[var(--accent)] text-[var(--accent-fg)]"
                    : "bg-black/40 text-white/80 hover:bg-black/60"
                }`}
              >
                {mode}
              </button>
            ))}
          </div>

          <div className="flex items-center gap-1.5">
            <button
              type="button"
              onClick={() => void handleCopyCode()}
              className="flex items-center gap-1 rounded-lg border border-white/15 bg-black/50 px-2.5 py-1 text-[10px] font-semibold text-white backdrop-blur-sm transition hover:bg-black/70"
            >
              {copiedCode ? (
                <Check className="h-3 w-3 text-emerald-400" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
              {copiedCode ? "Copied!" : "Copy Code"}
            </button>
            <button
              type="button"
              onClick={() => {
                onReset();
                if (crosshair.imgSrc) {
                  onUpdate("imgSrc", DEFAULT_CROSSHAIR_CONFIG.imgSrc);
                }
              }}
              className="flex items-center gap-1 rounded-lg border border-white/15 bg-black/50 px-2.5 py-1 text-[10px] font-semibold text-white backdrop-blur-sm transition hover:bg-black/70"
            >
              <RotateCcw className="h-3 w-3" />
              Reset
            </button>
          </div>
        </div>

        <CrosshairCanvas
          config={crosshair}
          firing={holdFiring}
          spreadSimulation={spreadSim}
          className="pointer-events-none absolute inset-0 z-10 block h-full w-full"
        />

        {/* Firing Simulation Toolbar */}
        <div className="relative z-20 flex flex-wrap items-center justify-between gap-2 p-2.5">
          <div className="flex items-center gap-1 rounded-lg bg-black/50 p-1 backdrop-blur-sm">
            <span className="px-1.5 text-[10px] font-semibold text-white/75">
              Fire Sim:
            </span>
            {simModes.map((m) => (
              <button
                key={m.id}
                type="button"
                onClick={() => setSpreadSim(m.id)}
                className={`rounded px-2 py-0.5 text-[10px] font-bold transition ${
                  spreadSim === m.id
                    ? "bg-[var(--accent)] text-[var(--accent-fg)]"
                    : "text-white/80 hover:bg-white/10"
                }`}
              >
                {m.label}
              </button>
            ))}
          </div>

          <button
            type="button"
            onPointerDown={() => setHoldFiring(true)}
            onPointerUp={() => setHoldFiring(false)}
            onPointerLeave={() => setHoldFiring(false)}
            onPointerCancel={() => setHoldFiring(false)}
            className={`flex items-center gap-1.5 rounded-lg border px-3 py-1 text-[10px] font-bold transition select-none ${
              holdFiring
                ? "border-rose-400 bg-rose-500 text-white shadow-[0_0_16px_rgba(244,63,94,0.6)]"
                : "border-white/15 bg-black/55 text-white hover:bg-black/75"
            }`}
          >
            <Flame className="h-3 w-3" />
            {holdFiring ? "Firing..." : "Hold to Fire"}
          </button>
        </div>
      </PanelCard>

      {/* Top 2 Cards: Geometry/Offset + Dot/Circle/Surface */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <PanelCard surfaceMode={surfaceMode} className="space-y-3.5 p-4">
          <div className="flex items-center justify-between">
            <h2 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
              Geometry & Center Offset
            </h2>
            {(crosshair.offsetX !== 0 || crosshair.offsetY !== 0) && (
              <button
                type="button"
                onClick={() => {
                  onUpdate("offsetX", 0);
                  onUpdate("offsetY", 0);
                }}
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="flex items-center gap-1 rounded-md border px-2 py-0.5 text-[10px] font-semibold transition hover:border-[var(--accent)]"
              >
                <Crosshair className="h-3 w-3" />
                Center (0,0)
              </button>
            )}
          </div>

          <SliderRow
            label="Length"
            value={crosshair.length}
            unit="px"
            {...CROSSHAIR_RANGES.length}
            onChange={(v) => onUpdate("length", v)}
          />
          <SliderRow
            label="Thickness"
            value={crosshair.thickness}
            unit="px"
            {...CROSSHAIR_RANGES.thickness}
            onChange={(v) => onUpdate("thickness", v)}
          />
          <SliderRow
            label="Center Gap"
            value={crosshair.gap}
            unit="px"
            {...CROSSHAIR_RANGES.gap}
            onChange={(v) => onUpdate("gap", v)}
          />
          <SliderRow
            label="Rotation"
            value={crosshair.rotation}
            unit="°"
            {...CROSSHAIR_RANGES.rotation}
            onChange={(v) => onUpdate("rotation", v)}
          />

          <div className="grid grid-cols-2 gap-3">
            <SliderRow
              label="Center Offset X"
              value={crosshair.offsetX}
              unit="px"
              {...CROSSHAIR_RANGES.offsetX}
              onChange={(v) => onUpdate("offsetX", v)}
            />
            <SliderRow
              label="Center Offset Y"
              value={crosshair.offsetY}
              unit="px"
              {...CROSSHAIR_RANGES.offsetY}
              onChange={(v) => onUpdate("offsetY", v)}
            />
          </div>

          <div
            style={{ borderColor: "var(--border-subtle)" }}
            className="grid grid-cols-2 gap-2 border-t pt-3"
          >
            <ToggleChip
              label="T-Shape (No Top)"
              checked={crosshair.tShape}
              onChange={(v) => onUpdate("tShape", v)}
            />
            <ToggleChip
              label="Dot Only (Hide Bars)"
              checked={crosshair.hideBars}
              onChange={(v) => onUpdate("hideBars", v)}
            />
            <ToggleChip
              label="Top Bar"
              checked={crosshair.showTop && !crosshair.tShape}
              onChange={(v) => {
                if (v && crosshair.tShape) onUpdate("tShape", false);
                onUpdate("showTop", v);
              }}
            />
            <ToggleChip
              label="Bottom Bar"
              checked={crosshair.showBottom}
              onChange={(v) => onUpdate("showBottom", v)}
            />
            <ToggleChip
              label="Left Bar"
              checked={crosshair.showLeft}
              onChange={(v) => onUpdate("showLeft", v)}
            />
            <ToggleChip
              label="Right Bar"
              checked={crosshair.showRight}
              onChange={(v) => onUpdate("showRight", v)}
            />
          </div>
        </PanelCard>

        <PanelCard surfaceMode={surfaceMode} className="space-y-3.5 p-4">
          <h2 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Dot, Circle, Outline & Surface
          </h2>

          <div className="grid grid-cols-2 gap-2">
            <ToggleChip
              label="Center Dot"
              checked={crosshair.dot}
              onChange={(v) => onUpdate("dot", v)}
            />
            <ToggleChip
              label="Circle Ring"
              checked={crosshair.hasCircle}
              onChange={(v) => onUpdate("hasCircle", v)}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <SliderRow
              label="Dot Size"
              value={crosshair.dotSize}
              unit="px"
              {...CROSSHAIR_RANGES.dotSize}
              onChange={(v) => onUpdate("dotSize", v)}
            />
            <div className="space-y-1.5">
              <span className="block text-[11px] font-medium text-[var(--muted-text)]">
                Dot Shape
              </span>
              <div
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="grid grid-cols-2 rounded-lg border p-0.5"
              >
                {(["circle", "square"] as CrosshairDotShape[]).map((shape) => {
                  const active = crosshair.dotShape === shape;
                  return (
                    <button
                      key={shape}
                      type="button"
                      onClick={() => onUpdate("dotShape", shape)}
                      style={{
                        backgroundColor: active ? "var(--accent)" : "transparent",
                        color: active ? "var(--accent-fg)" : "var(--text-color)",
                      }}
                      className="rounded-md py-1 text-[10px] font-bold capitalize transition"
                    >
                      {shape}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>

          {crosshair.hasCircle && (
            <SliderRow
              label="Circle Radius"
              value={crosshair.circleRadius}
              unit="px"
              {...CROSSHAIR_RANGES.circleRadius}
              onChange={(v) => onUpdate("circleRadius", v)}
            />
          )}

          <SliderRow
            label="Crosshair Opacity"
            value={Math.round(crosshair.opacity * 100)}
            unit="%"
            min={10}
            max={100}
            step={5}
            onChange={(v) => onUpdate("opacity", Number((v / 100).toFixed(2)))}
          />

          <SliderRow
            label="Blur / Soft Glow"
            value={crosshair.blur}
            unit="px"
            {...CROSSHAIR_RANGES.blur}
            onChange={(v) => onUpdate("blur", v)}
          />

          <div
            style={{ borderColor: "var(--border-subtle)" }}
            className="space-y-3 border-t pt-3"
          >
            <ToggleChip
              label="Enable Outline / Border"
              checked={crosshair.outline}
              onChange={(v) => onUpdate("outline", v)}
            />
            <div className="grid grid-cols-2 gap-3">
              <SliderRow
                label="Outline Thickness"
                value={crosshair.outlineThickness}
                unit="px"
                {...CROSSHAIR_RANGES.outlineThickness}
                onChange={(v) => onUpdate("outlineThickness", v)}
              />
              <SliderRow
                label="Outline Opacity"
                value={Math.round(crosshair.outlineOpacity * 100)}
                unit="%"
                min={0}
                max={100}
                step={5}
                onChange={(v) =>
                  onUpdate("outlineOpacity", Number((v / 100).toFixed(2)))
                }
              />
            </div>
          </div>
        </PanelCard>
      </div>

      {/* Bottom 2 Cards: Color/Gradient/Recoil + Dynamic Spread/Motion */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <PanelCard surfaceMode={surfaceMode} className="space-y-3.5 p-4">
          <h2 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Color, Gradient & Dynamic Recoil
          </h2>

          <div
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
            }}
            className="grid grid-cols-3 rounded-xl border p-1"
          >
            {(["solid", "gradient", "rainbow"] as CrosshairColorMode[]).map(
              (mode) => {
                const active = crosshair.colorMode === mode;
                return (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => onUpdate("colorMode", mode)}
                    style={{
                      backgroundColor: active ? "var(--accent)" : "transparent",
                      color: active ? "var(--accent-fg)" : "var(--text-color)",
                    }}
                    className="rounded-lg py-1.5 text-[11px] font-bold capitalize transition"
                  >
                    {mode}
                  </button>
                );
              },
            )}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <ColorPickerField
              label={crosshair.colorMode === "gradient" ? "Gradient Start" : "Inner Color"}
              value={crosshair.color}
              onChange={(v) => onUpdate("color", v)}
            />
            <ColorPickerField
              label="Outline Color"
              value={crosshair.outlineColor}
              onChange={(v) => onUpdate("outlineColor", v)}
            />
          </div>

          {crosshair.colorMode === "gradient" && (
            <div className="grid grid-cols-2 gap-3">
              <ColorPickerField
                label="Gradient End"
                value={crosshair.gradientColor}
                onChange={(v) => onUpdate("gradientColor", v)}
              />
              <SliderRow
                label="Gradient Angle"
                value={crosshair.gradientAngle}
                unit="°"
                {...CROSSHAIR_RANGES.gradientAngle}
                onChange={(v) => onUpdate("gradientAngle", v)}
              />
            </div>
          )}

          {crosshair.colorMode === "rainbow" && (
            <SliderRow
              label="RGB Cycle Speed"
              value={crosshair.rainbowSpeed}
              {...CROSSHAIR_RANGES.rainbowSpeed}
              onChange={(v) => onUpdate("rainbowSpeed", v)}
            />
          )}

          <div
            style={{ borderColor: "var(--border-subtle)" }}
            className="grid grid-cols-2 items-end gap-3 border-t pt-3"
          >
            <ToggleChip
              label="Dynamic Recoil Color"
              checked={crosshair.recoilColorEnabled}
              onChange={(v) => onUpdate("recoilColorEnabled", v)}
            />
            <ColorPickerField
              label="Recoil Peak Color"
              value={crosshair.recoilColor}
              onChange={(v) => onUpdate("recoilColor", v)}
            />
          </div>
        </PanelCard>

        <PanelCard surfaceMode={surfaceMode} className="space-y-3.5 p-4">
          <h2 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Firing Spread, Motion & Custom Image
          </h2>

          <ToggleChip
            label="Dynamic Firing Spread (Bloom on Fire / Sim)"
            checked={crosshair.dynamicSpread}
            onChange={(v) => onUpdate("dynamicSpread", v)}
          />

          <div className="grid grid-cols-2 gap-3">
            <SliderRow
              label="Spread Expansion"
              value={crosshair.spreadAmount}
              unit="px"
              {...CROSSHAIR_RANGES.spreadAmount}
              onChange={(v) => onUpdate("spreadAmount", v)}
            />
            <SliderRow
              label="Spread / Recovery Speed"
              value={crosshair.spreadSpeed}
              {...CROSSHAIR_RANGES.spreadSpeed}
              onChange={(v) => onUpdate("spreadSpeed", v)}
            />
          </div>

          <div
            style={{ borderColor: "var(--border-subtle)" }}
            className="space-y-2.5 border-t pt-3"
          >
            <ToggleChip
              label="Spin Rotation Animation"
              checked={crosshair.animate}
              onChange={(v) => onUpdate("animate", v)}
            />
            {crosshair.animate && (
              <SliderRow
                label="Spin Speed"
                value={crosshair.spinSpeed}
                {...CROSSHAIR_RANGES.spinSpeed}
                onChange={(v) => onUpdate("spinSpeed", v)}
              />
            )}
          </div>

          <div
            style={{ borderColor: "var(--border-subtle)" }}
            className="space-y-2 border-t pt-3"
          >
            <div className="flex gap-2">
              <input
                ref={fileInputRef}
                type="file"
                accept=".png,.jpg,.jpeg,.svg,.webp"
                onChange={handleImageUpload}
                className="hidden"
              />
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border px-3 py-2 text-[11px] font-semibold transition hover:border-[var(--accent)]"
              >
                <Upload className="h-3.5 w-3.5 text-[var(--accent)]" />
                {crosshair.imgSrc ? "Replace Custom Image" : "Upload Custom Image"}
              </button>
              {crosshair.imgSrc && (
                <button
                  type="button"
                  onClick={() => onUpdate("imgSrc", null)}
                  className="rounded-lg border border-red-500/30 bg-red-500/15 px-3 py-2 text-red-400 transition hover:bg-red-500/25"
                  title="Clear Custom Image"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>

            <label className="flex cursor-pointer items-center gap-2 pt-1 text-[11px] font-semibold text-rose-400">
              <input
                type="checkbox"
                checked={crosshair.hideCrosshair}
                onChange={(e) => onUpdate("hideCrosshair", e.target.checked)}
              />
              <span className="flex items-center gap-1.5">
                {crosshair.hideCrosshair ? (
                  <EyeOff className="h-3.5 w-3.5" />
                ) : (
                  <Eye className="h-3.5 w-3.5" />
                )}
                Hide Crosshair Manually
              </span>
            </label>
          </div>
        </PanelCard>
      </div>
    </div>
  );
}
