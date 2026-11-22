import { useRef, type MouseEvent, type ReactNode } from "react";
import type { ThemeConfig } from "../types/themes";

interface DynamicGlassShellProps {
  theme: ThemeConfig;
  isMaximized?: boolean;
  children: ReactNode;
}

export function DynamicGlassShell({
  theme,
  isMaximized = false,
  children,
}: DynamicGlassShellProps) {
  const shellRef = useRef<HTMLDivElement | null>(null);
  const isGlass = theme.surfaceMode === "liquid-glass";

  const handleMouseMove = (e: MouseEvent<HTMLDivElement>) => {
    if (!isGlass || !theme.dynamicLighting || !shellRef.current) return;
    const rect = shellRef.current.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const xPct = ((e.clientX - rect.left) / rect.width) * 100;
    const yPct = ((e.clientY - rect.top) / rect.height) * 100;
    shellRef.current.style.setProperty("--pointer-x", `${xPct.toFixed(1)}%`);
    shellRef.current.style.setProperty("--pointer-y", `${yPct.toFixed(1)}%`);
  };

  return (
    <div
      ref={shellRef}
      onMouseMove={handleMouseMove}
      style={{
        borderRadius: isMaximized ? "0px" : "var(--radius)",
        color: "var(--text-color)",
      }}
      className={`relative flex h-full w-full flex-col overflow-hidden transition-[border-radius] duration-200 ${
        isGlass ? "luair-glass-shell" : "luair-solid-shell"
      }`}
    >
      {/* Ambient Aurora Orbs + Dynamic Cursor Lighting for Liquid Glass */}
      {isGlass && (
        <>
          <div
            className="pointer-events-none absolute -top-24 -left-24 h-72 w-72 rounded-full opacity-25 blur-3xl transition-colors duration-500"
            style={{ background: "var(--accent)" }}
            aria-hidden="true"
          />
          <div
            className="pointer-events-none absolute -right-24 -bottom-24 h-80 w-80 rounded-full opacity-20 blur-3xl transition-colors duration-500"
            style={{ background: "var(--highlight)" }}
            aria-hidden="true"
          />
          {theme.dynamicLighting && (
            <div
              className="luair-dynamic-light pointer-events-none absolute inset-0 transition-opacity duration-300"
              aria-hidden="true"
            />
          )}
          <div className="luair-refraction-ring z-30" aria-hidden="true" />
        </>
      )}

      <div className="relative z-10 flex min-h-0 flex-1 flex-col">{children}</div>
    </div>
  );
}
