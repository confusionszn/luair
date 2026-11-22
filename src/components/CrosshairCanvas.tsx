import { useEffect, useRef } from "react";
import {
  CrosshairRenderer,
  type SpreadSimMode,
} from "../engine/crosshairRenderer";
import type { CrosshairConfig } from "../types/crosshair";

interface CrosshairCanvasProps {
  config: CrosshairConfig;
  firing?: boolean;
  spreadSimulation?: SpreadSimMode;
  className?: string;
}

export function CrosshairCanvas({
  config,
  firing = false,
  spreadSimulation = "off",
  className = "pointer-events-none fixed inset-0 z-10 block h-screen w-screen",
}: CrosshairCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<CrosshairRenderer | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const renderer = new CrosshairRenderer(canvas, config);
    renderer.setFiring(firing);
    renderer.setSpreadSimulation(spreadSimulation);
    rendererRef.current = renderer;

    return () => {
      renderer.destroy();
      rendererRef.current = null;
    };
    // Mount once; updates flow through the dedicated effect hooks below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    rendererRef.current?.updateConfig(config);
  }, [config]);

  useEffect(() => {
    rendererRef.current?.setFiring(firing);
  }, [firing]);

  useEffect(() => {
    rendererRef.current?.setSpreadSimulation(spreadSimulation);
  }, [spreadSimulation]);

  return <canvas ref={canvasRef} className={className} />;
}
