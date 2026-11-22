import type { ReactNode } from "react";
import type { ThemeSurfaceMode } from "../types/themes";

interface PanelCardProps {
  surfaceMode: ThemeSurfaceMode;
  className?: string;
  children: ReactNode;
}

export function PanelCard({
  surfaceMode,
  className = "",
  children,
}: PanelCardProps) {
  return (
    <section
      style={{ borderRadius: "calc(var(--radius) * 0.85)" }}
      className={`${
        surfaceMode === "liquid-glass" ? "luair-glass-panel" : "luair-solid-panel"
      } ${className}`}
    >
      {children}
    </section>
  );
}

interface SliderRowProps {
  label: string;
  value: number;
  unit?: string;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}

export function SliderRow({
  label,
  value,
  unit = "",
  min,
  max,
  step,
  onChange,
}: SliderRowProps) {
  return (
    <label className="block space-y-1.5">
      <div className="flex items-center justify-between text-[11px]">
        <span className="font-medium text-[var(--muted-text)]">{label}</span>
        <span className="font-mono font-bold text-[var(--accent)]">
          {value}
          {unit}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-1.5 w-full cursor-pointer rounded-full"
        style={{ backgroundColor: "var(--border-subtle)" }}
      />
    </label>
  );
}

interface ToggleChipProps {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

export function ToggleChip({ label, checked, onChange }: ToggleChipProps) {
  return (
    <label
      style={{
        backgroundColor: checked ? "rgba(var(--accent-rgb), 0.14)" : "var(--surface-subtle)",
        borderColor: checked ? "rgba(var(--accent-rgb), 0.45)" : "var(--border-subtle)",
      }}
      className="flex cursor-pointer items-center gap-2 rounded-lg border px-2.5 py-2 text-[11px] font-medium transition-colors"
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="truncate">{label}</span>
    </label>
  );
}
