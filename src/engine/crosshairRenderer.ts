import type { CrosshairConfig } from "../types/crosshair";
import { hexToRgb } from "../types/themes";

const FRAME_MS_60FPS = 1000 / 60;

export type SpreadSimMode = "off" | "tap" | "burst" | "spray";

function lerpChannel(a: number, b: number, t: number): number {
  return Math.round(a + (b - a) * t);
}

function blendHexColors(hexA: string, hexB: string, t: number): string {
  if (t <= 0.001) return hexA;
  if (t >= 0.999) return hexB;
  const c1 = hexToRgb(hexA);
  const c2 = hexToRgb(hexB);
  const r = lerpChannel(c1.r, c2.r, t);
  const g = lerpChannel(c1.g, c2.g, t);
  const b = lerpChannel(c1.b, c2.b, t);
  return `rgb(${r}, ${g}, ${b})`;
}

function hslToHex(h: number, s: number, l: number): string {
  const sNorm = s / 100;
  const lNorm = l / 100;
  const k = (n: number) => (n + h / 30) % 12;
  const a = sNorm * Math.min(lNorm, 1 - lNorm);
  const f = (n: number) => {
    const color = lNorm - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
    return Math.round(255 * color)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/**
 * High-performance Canvas 2D crosshair renderer supporting Crosshair X geometry,
 * T-shape, center offset, blur, opacity, gradients, dynamic spread, and recoil coloring.
 * Uses 0% CPU when static and idle.
 */
export class CrosshairRenderer {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private config: CrosshairConfig;
  private spinAngle = 0;
  private rainbowHue = 0;
  private recoilHeat = 0;
  private isFiring = false;
  private spreadSim: SpreadSimMode = "off";
  private simClockMs = 0;
  private rafId: number | null = null;
  private lastTimestamp: number | null = null;
  private cssWidth = 0;
  private cssHeight = 0;
  private customImage: HTMLImageElement | null = null;
  private loadedImgSrc: string | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private readonly handleWindowResize = (): void => {
    this.resize();
  };

  constructor(canvas: HTMLCanvasElement, initialConfig: CrosshairConfig) {
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Unable to acquire 2D rendering context for crosshair canvas.");
    }
    this.canvas = canvas;
    this.ctx = context;
    this.config = { ...initialConfig };

    this.syncCustomImage();
    this.resize();

    if (typeof ResizeObserver !== "undefined") {
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(canvas);
    }
    window.addEventListener("resize", this.handleWindowResize);
    this.syncAnimationLoop();
  }

  resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.clientWidth || window.innerWidth;
    const height = this.canvas.clientHeight || window.innerHeight;

    this.cssWidth = width;
    this.cssHeight = height;
    this.canvas.width = Math.max(1, Math.round(width * dpr));
    this.canvas.height = Math.max(1, Math.round(height * dpr));

    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.drawFrame(0);
  }

  updateConfig(next: CrosshairConfig): void {
    const wasAnimating = this.config.animate;
    this.config = { ...next };

    if (wasAnimating && !this.config.animate) {
      this.spinAngle = 0;
    }
    if (this.config.colorMode !== "rainbow" && !this.config.rainbow) {
      this.rainbowHue = 0;
    }

    this.syncCustomImage();
    this.drawFrame(0);
    this.syncAnimationLoop();
  }

  setFiring(active: boolean): void {
    if (this.isFiring === active) return;
    this.isFiring = active;
    this.syncAnimationLoop();
  }

  setSpreadSimulation(mode: SpreadSimMode): void {
    if (this.spreadSim === mode) return;
    this.spreadSim = mode;
    this.simClockMs = 0;
    this.syncAnimationLoop();
  }

  drawFrame(dtMs = 0): void {
    const { ctx, cssWidth, cssHeight, config } = this;
    ctx.clearRect(0, 0, cssWidth, cssHeight);

    if (config.hideCrosshair || cssWidth === 0 || cssHeight === 0) {
      return;
    }

    const dtSec = dtMs > 0 ? Math.min(dtMs / 1000, 0.1) : 0;
    const dtScale = dtMs > 0 ? Math.min(dtMs / FRAME_MS_60FPS, 4) : 0;

    if (config.animate) {
      this.spinAngle = (this.spinAngle + config.spinSpeed * dtScale) % 360;
    }

    const isRainbow = config.colorMode === "rainbow" || config.rainbow;
    if (isRainbow) {
      this.rainbowHue = (this.rainbowHue + config.rainbowSpeed * dtScale) % 360;
    }

    // Evaluate live firing or simulation firing state.
    if (dtMs > 0 && this.spreadSim !== "off") {
      this.simClockMs += dtMs;
    }
    const simFiring = this.isSimFiring();
    const activeFire = this.isFiring || simFiring;

    if (dtSec > 0) {
      if (activeFire) {
        const riseRate = 4 + config.spreadSpeed * 0.9;
        this.recoilHeat = 1 - (1 - this.recoilHeat) * Math.exp(-riseRate * dtSec);
      } else if (this.recoilHeat > 0) {
        const decayRate = 3 + config.spreadSpeed * 0.55;
        this.recoilHeat *= Math.exp(-decayRate * dtSec);
        if (this.recoilHeat < 0.002) {
          this.recoilHeat = 0;
        }
      }
    }

    const cx = Math.floor(cssWidth / 2) + config.offsetX + 0.5;
    const cy = Math.floor(cssHeight / 2) + config.offsetY + 0.5;

    const baseAlpha = Math.min(1, Math.max(0.05, config.opacity));
    const outlineAlpha = baseAlpha * Math.min(1, Math.max(0, config.outlineOpacity));

    const spreadOffset = config.dynamicSpread
      ? this.recoilHeat * config.spreadAmount
      : 0;
    const effectiveGap = config.gap + spreadOffset;
    const effectiveCircleRadius = config.circleRadius + spreadOffset * 0.85;
    const extent = Math.max(
      24,
      effectiveGap + config.length,
      config.hasCircle ? effectiveCircleRadius : 0,
    );

    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate(((config.rotation + this.spinAngle) * Math.PI) / 180);
    ctx.filter = config.blur > 0 ? `blur(${config.blur}px)` : "none";

    if (config.imgSrc) {
      const img = this.customImage;
      if (img && img.complete && img.naturalWidth > 0) {
        const drawW = config.length * 2 || img.naturalWidth;
        const drawH = config.length * 2 || img.naturalHeight;
        ctx.globalAlpha = baseAlpha;
        ctx.drawImage(img, -drawW / 2, -drawH / 2, drawW, drawH);
      }
      ctx.restore();
      return;
    }

    const mainFill = this.resolveFillStyle(extent);

    const drawBar = (x: number, y: number, w: number, h: number): void => {
      if (config.outline && config.outlineThickness > 0 && outlineAlpha > 0) {
        ctx.globalAlpha = outlineAlpha;
        ctx.fillStyle = config.outlineColor;
        ctx.fillRect(
          x - config.outlineThickness,
          y - config.outlineThickness,
          w + config.outlineThickness * 2,
          h + config.outlineThickness * 2,
        );
      }
      ctx.globalAlpha = baseAlpha;
      ctx.fillStyle = mainFill;
      ctx.fillRect(x, y, w, h);
    };

    const tOff = config.thickness / 2;
    const drawTop = config.showTop && !config.tShape;

    if (!config.hideBars && config.length > 0) {
      if (config.showLeft) {
        drawBar(-effectiveGap - config.length, -tOff, config.length, config.thickness);
      }
      if (config.showRight) {
        drawBar(effectiveGap, -tOff, config.length, config.thickness);
      }
      if (config.showBottom) {
        drawBar(-tOff, effectiveGap, config.thickness, config.length);
      }
      if (drawTop) {
        drawBar(-tOff, -effectiveGap - config.length, config.thickness, config.length);
      }
    }

    if (config.dot && config.dotSize > 0) {
      const halfDot = config.dotSize / 2;
      if (config.dotShape === "square") {
        drawBar(-halfDot, -halfDot, config.dotSize, config.dotSize);
      } else {
        if (config.outline && config.outlineThickness > 0 && outlineAlpha > 0) {
          ctx.beginPath();
          ctx.arc(0, 0, halfDot + config.outlineThickness, 0, 2 * Math.PI, false);
          ctx.globalAlpha = outlineAlpha;
          ctx.fillStyle = config.outlineColor;
          ctx.fill();
        }
        ctx.beginPath();
        ctx.arc(0, 0, halfDot, 0, 2 * Math.PI, false);
        ctx.globalAlpha = baseAlpha;
        ctx.fillStyle = mainFill;
        ctx.fill();
      }
    }

    if (config.hasCircle && effectiveCircleRadius > 0) {
      ctx.beginPath();
      ctx.arc(0, 0, effectiveCircleRadius, 0, 2 * Math.PI, false);
      if (config.outline && config.outlineThickness > 0 && outlineAlpha > 0) {
        ctx.globalAlpha = outlineAlpha;
        ctx.lineWidth = config.thickness + config.outlineThickness * 2;
        ctx.strokeStyle = config.outlineColor;
        ctx.stroke();
      }
      ctx.globalAlpha = baseAlpha;
      ctx.lineWidth = config.thickness;
      ctx.strokeStyle = mainFill;
      ctx.stroke();
    }

    ctx.restore();
  }

  destroy(): void {
    this.stopAnimationLoop();
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    window.removeEventListener("resize", this.handleWindowResize);
  }

  private resolveFillStyle(extent: number): string | CanvasGradient {
    const { config, ctx } = this;
    const recoilBlend = config.recoilColorEnabled ? this.recoilHeat : 0;

    if (config.colorMode === "rainbow" || config.rainbow) {
      const rainbowHex = hslToHex(this.rainbowHue, 100, 50);
      return blendHexColors(rainbowHex, config.recoilColor, recoilBlend);
    }

    if (config.colorMode === "gradient") {
      const rad = (config.gradientAngle * Math.PI) / 180;
      const dx = Math.cos(rad) * extent;
      const dy = Math.sin(rad) * extent;
      const grad = ctx.createLinearGradient(-dx, -dy, dx, dy);
      grad.addColorStop(0, blendHexColors(config.color, config.recoilColor, recoilBlend));
      grad.addColorStop(
        1,
        blendHexColors(config.gradientColor, config.recoilColor, recoilBlend),
      );
      return grad;
    }

    return blendHexColors(config.color, config.recoilColor, recoilBlend);
  }

  private isSimFiring(): boolean {
    if (this.spreadSim === "spray") return true;
    if (this.spreadSim === "tap") {
      return this.simClockMs % 700 < 140;
    }
    if (this.spreadSim === "burst") {
      const t = this.simClockMs % 950;
      return (t >= 0 && t < 110) || (t >= 170 && t < 280) || (t >= 340 && t < 450);
    }
    return false;
  }

  private syncCustomImage(): void {
    const src = this.config.imgSrc;
    if (!src) {
      this.loadedImgSrc = null;
      this.customImage = null;
      return;
    }

    if (this.loadedImgSrc === src && this.customImage) {
      return;
    }

    this.loadedImgSrc = src;
    const img = new Image();
    img.onload = () => {
      if (this.loadedImgSrc === src) {
        this.drawFrame(0);
      }
    };
    img.src = src;
    this.customImage = img;
  }

  private syncAnimationLoop(): void {
    const needsAnimation =
      !this.config.hideCrosshair &&
      (this.config.animate ||
        this.config.colorMode === "rainbow" ||
        this.config.rainbow ||
        this.spreadSim !== "off" ||
        this.isFiring ||
        this.recoilHeat > 0.002);

    if (needsAnimation && this.rafId === null) {
      this.lastTimestamp = null;
      const tick = (timestamp: number): void => {
        const dtMs =
          this.lastTimestamp === null ? FRAME_MS_60FPS : timestamp - this.lastTimestamp;
        this.lastTimestamp = timestamp;
        this.drawFrame(dtMs);

        const keepRunning =
          !this.config.hideCrosshair &&
          (this.config.animate ||
            this.config.colorMode === "rainbow" ||
            this.config.rainbow ||
            this.spreadSim !== "off" ||
            this.isFiring ||
            this.recoilHeat > 0.002);

        if (keepRunning) {
          this.rafId = window.requestAnimationFrame(tick);
        } else {
          this.rafId = null;
          this.lastTimestamp = null;
        }
      };
      this.rafId = window.requestAnimationFrame(tick);
    } else if (!needsAnimation && this.rafId !== null) {
      this.stopAnimationLoop();
    }
  }

  private stopAnimationLoop(): void {
    if (this.rafId !== null) {
      window.cancelAnimationFrame(this.rafId);
      this.rafId = null;
      this.lastTimestamp = null;
    }
  }
}
