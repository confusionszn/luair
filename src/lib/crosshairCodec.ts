import { parseCrosshairConfig, type CrosshairConfig } from "../types/crosshair";

export const CROSSHAIR_CODE_PREFIX = "LUAIR-CH1:";

interface ExportedCrosshairEnvelope {
  schema: "luair-crosshair";
  version: 2;
  name?: string;
  crosshair: CrosshairConfig;
}

export type CrosshairImportResult =
  | {
      ok: true;
      config: CrosshairConfig;
      name?: string;
      format: "code" | "json";
    }
  | {
      ok: false;
      error: string;
    };

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

function hasCrosshairPayloadKeys(obj: Record<string, unknown>): boolean {
  const candidate =
    typeof obj.crosshair === "object" && obj.crosshair !== null
      ? (obj.crosshair as Record<string, unknown>)
      : typeof obj.data === "object" && obj.data !== null
        ? (obj.data as Record<string, unknown>)
        : obj;

  const knownKeys = [
    "color",
    "length",
    "thickness",
    "gap",
    "dot",
    "dotSize",
    "tShape",
    "hasCircle",
    "circleRadius",
    "outline",
    "colorMode",
    "imgSrc",
  ];
  return knownKeys.some((key) => key in candidate);
}

export function encodeCrosshairCode(config: CrosshairConfig, name?: string): string {
  const normalized = parseCrosshairConfig(config);
  const payload = name?.trim()
    ? { name: name.trim(), ...normalized }
    : normalized;
  const json = JSON.stringify(payload);
  return `${CROSSHAIR_CODE_PREFIX}${bytesToBase64Url(new TextEncoder().encode(json))}`;
}

export function exportCrosshairJson(config: CrosshairConfig, name?: string): string {
  const envelope: ExportedCrosshairEnvelope = {
    schema: "luair-crosshair",
    version: 2,
    ...(name?.trim() ? { name: name.trim() } : {}),
    crosshair: parseCrosshairConfig(config),
  };
  return JSON.stringify(envelope, null, 2);
}

export function importCrosshairInput(raw: string): CrosshairImportResult {
  const trimmed = raw.trim();
  if (!trimmed) {
    return {
      ok: false,
      error: "Paste a LUAIR-CH1: Base64 share code or a .luair / .json configuration.",
    };
  }

  if (trimmed.toUpperCase().startsWith(CROSSHAIR_CODE_PREFIX)) {
    const encoded = trimmed.slice(CROSSHAIR_CODE_PREFIX.length).trim();
    if (!encoded) {
      return {
        ok: false,
        error: "Share code is empty after LUAIR-CH1: prefix.",
      };
    }
    try {
      const jsonStr = new TextDecoder().decode(base64UrlToBytes(encoded));
      const parsed: unknown = JSON.parse(jsonStr);
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        return {
          ok: false,
          error: "Decoded share code does not contain a valid crosshair object.",
        };
      }
      const record = parsed as Record<string, unknown>;
      const name =
        typeof record.name === "string" && record.name.trim().length > 0
          ? record.name.trim()
          : undefined;
      return {
        ok: true,
        config: parseCrosshairConfig(record),
        name,
        format: "code",
      };
    } catch {
      return {
        ok: false,
        error: "Invalid or corrupted LUAIR-CH1: Base64 share code.",
      };
    }
  }

  // Try JSON first.
  if (trimmed.startsWith("{")) {
    try {
      const parsed: unknown = JSON.parse(trimmed);
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        return { ok: false, error: "Crosshair JSON must be an object." };
      }
      const record = parsed as Record<string, unknown>;
      if (!hasCrosshairPayloadKeys(record)) {
        return {
          ok: false,
          error: "JSON object does not contain recognizable crosshair settings.",
        };
      }
      const name =
        typeof record.name === "string" && record.name.trim().length > 0
          ? record.name.trim()
          : undefined;
      return {
        ok: true,
        config: parseCrosshairConfig(record),
        name,
        format: "json",
      };
    } catch {
      return { ok: false, error: "Invalid JSON syntax." };
    }
  }

  // Fallback: try raw Base64-encoded JSON string without prefix.
  try {
    const jsonStr = new TextDecoder().decode(base64UrlToBytes(trimmed));
    const parsed: unknown = JSON.parse(jsonStr);
    if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
      const record = parsed as Record<string, unknown>;
      if (hasCrosshairPayloadKeys(record)) {
        const name =
          typeof record.name === "string" && record.name.trim().length > 0
            ? record.name.trim()
            : undefined;
        return {
          ok: true,
          config: parseCrosshairConfig(record),
          name,
          format: "code",
        };
      }
    }
  } catch {
    // Fall through to unified error message.
  }

  return {
    ok: false,
    error: "Unrecognized format. Provide a LUAIR-CH1: code, Base64 string, or JSON.",
  };
}
