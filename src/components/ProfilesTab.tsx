import { useRef, useState, type ChangeEvent } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Check,
  Copy,
  Download,
  FileUp,
  Mouse,
  Plus,
  Save,
  Trash2,
  Upload,
} from "lucide-react";
import { CrosshairCanvas } from "./CrosshairCanvas";
import { PanelCard } from "./UiPrimitives";
import {
  encodeCrosshairCode,
  exportCrosshairJson,
  importCrosshairInput,
} from "../lib/crosshairCodec";
import type { CrosshairConfig, CrosshairProfile } from "../types/crosshair";
import type { ThemeSurfaceMode } from "../types/themes";

interface ProfilesTabProps {
  profiles: CrosshairProfile[];
  activeProfileId: string | null;
  currentCrosshair: CrosshairConfig;
  enableProfileHotkeys: boolean;
  recordingProfileId: string | null;
  switchedBanner: string | null;
  surfaceMode: ThemeSurfaceMode;
  onToggleProfileHotkeys: (enabled: boolean) => void;
  onAddProfile: (name: string) => void;
  onSelectProfile: (id: string) => void;
  onSaveOverProfile: (id: string) => void;
  onRenameProfile: (id: string, name: string) => void;
  onDeleteProfile: (id: string) => void;
  onStartRecording: (profileId: string) => void;
  onManualBind: (profileId: string, hotkey: string | null) => void;
  onImportCrosshair: (config: CrosshairConfig) => void;
  onImportAsProfile: (config: CrosshairConfig, name: string) => void;
}

const MOUSE_QUICK_BINDS = [
  "Mouse4",
  "Mouse5",
  "MouseMiddle",
  "MouseRight",
  "MouseLeft",
];

export function ProfilesTab({
  profiles,
  activeProfileId,
  currentCrosshair,
  enableProfileHotkeys,
  recordingProfileId,
  switchedBanner,
  surfaceMode,
  onToggleProfileHotkeys,
  onAddProfile,
  onSelectProfile,
  onSaveOverProfile,
  onRenameProfile,
  onDeleteProfile,
  onStartRecording,
  onManualBind,
  onImportCrosshair,
  onImportAsProfile,
}: ProfilesTabProps) {
  const [newProfileName, setNewProfileName] = useState("");
  const [importText, setImportText] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<{
    kind: "ok" | "err";
    msg: string;
  } | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const activeProfile = profiles.find((p) => p.id === activeProfileId);
  const currentShareCode = encodeCrosshairCode(
    currentCrosshair,
    activeProfile?.name,
  );

  const handleAdd = () => {
    onAddProfile(newProfileName);
    setNewProfileName("");
  };

  const handleCopy = async (payload: string, id: string) => {
    try {
      await navigator.clipboard.writeText(payload);
      setCopiedId(id);
      window.setTimeout(() => {
        setCopiedId((cur) => (cur === id ? null : cur));
      }, 1700);
    } catch {
      setImportText(payload);
      setFeedback({
        kind: "ok",
        msg: "Clipboard unavailable — copied payload into the import box below.",
      });
    }
  };

  const handleDownloadFile = () => {
    const json = exportCrosshairJson(currentCrosshair, activeProfile?.name);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    const slug = (activeProfile?.name || "luair-crosshair")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-");
    anchor.href = url;
    anchor.download = `${slug}.luair`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const handleImportApply = (asNewSlot: boolean, raw = importText) => {
    const res = importCrosshairInput(raw);
    if (!res.ok) {
      setFeedback({ kind: "err", msg: res.error });
      return;
    }

    if (asNewSlot) {
      if (profiles.length >= 10) {
        setFeedback({
          kind: "err",
          msg: "All 10 slots are full — delete a slot or apply directly to current.",
        });
        return;
      }
      const slotName = res.name ?? `Imported ${profiles.length + 1}`;
      onImportAsProfile(res.config, slotName);
      setFeedback({
        kind: "ok",
        msg: `Created slot "${slotName}" from ${res.format === "code" ? "Base64 code" : "JSON file"}.`,
      });
    } else {
      onImportCrosshair(res.config);
      setFeedback({
        kind: "ok",
        msg: `Applied imported crosshair from ${res.format === "code" ? "Base64 code" : "JSON file"}.`,
      });
    }
    setImportText("");
  };

  const handleFileLoad = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const text = ev.target?.result;
      if (typeof text === "string") {
        handleImportApply(false, text);
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  };

  return (
    <div className="space-y-4">
      {/* Global Non-Blocking Pass-Through Banner */}
      <PanelCard
        surfaceMode={surfaceMode}
        className="flex flex-wrap items-center justify-between gap-3 p-4"
      >
        <div className="min-w-0 flex-1">
          <h2 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Non-blocking Keyboard & Mouse Slot Hotkeys
          </h2>
          <p className="mt-1 text-[11px] text-[var(--muted-text)]">
            Bind any keyboard key, modifier combo, or mouse button (Mouse4, Mouse5, Middle,
            Right). Inputs pass straight through to your game with zero delay.
          </p>
        </div>
        <label
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="flex shrink-0 cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-[11px] font-semibold"
        >
          <input
            type="checkbox"
            checked={enableProfileHotkeys}
            onChange={(e) => onToggleProfileHotkeys(e.target.checked)}
          />
          <span>Enabled</span>
        </label>
      </PanelCard>

      {/* New Slot Creator */}
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <div className="flex items-center justify-between">
          <h3 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            Designated Crosshair Slots ({profiles.length} / 10)
          </h3>
          {switchedBanner && (
            <span className="flex items-center gap-1 text-[10px] font-semibold text-emerald-400">
              <Check className="h-3 w-3" /> Switched via hotkey
            </span>
          )}
        </div>
        <div className="flex gap-2">
          <input
            type="text"
            value={newProfileName}
            onChange={(e) => setNewProfileName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && profiles.length < 10) {
                handleAdd();
              }
            }}
            placeholder="New slot name (e.g., Sheriff, AWP, Wingman)..."
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
              color: "var(--text-color)",
            }}
            className="flex-1 rounded-lg border px-3 py-2 text-xs outline-none focus:border-[var(--accent)]"
          />
          <button
            type="button"
            disabled={profiles.length >= 10}
            onClick={handleAdd}
            className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-4 py-2 text-xs font-bold text-[var(--accent-fg)] transition hover:opacity-90 disabled:opacity-40"
          >
            <Plus className="h-3.5 w-3.5" />
            Add Slot
          </button>
        </div>
      </PanelCard>

      {/* Designated Slots List */}
      <div className="space-y-2.5">
        <AnimatePresence initial={false}>
          {profiles.map((profile, idx) => {
            const isActive = profile.id === activeProfileId;
            const isRecordingThis = recordingProfileId === profile.id;

            return (
              <motion.div
                key={profile.id}
                layout
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.96 }}
                transition={{ duration: 0.18 }}
              >
                <PanelCard
                  surfaceMode={surfaceMode}
                  className={`space-y-2.5 p-3.5 transition-colors ${
                    isActive ? "ring-1 ring-[var(--accent)]" : ""
                  }`}
                >
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="flex min-w-0 flex-1 items-center gap-3">
                      <span
                        style={{
                          backgroundColor: "var(--surface-subtle)",
                          borderColor: "var(--border-subtle)",
                        }}
                        className="rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-bold text-[var(--muted-text)]"
                      >
                        #{idx + 1}
                      </span>

                      <div className="relative h-10 w-10 shrink-0 overflow-hidden rounded-lg border border-white/15 bg-[#06080f]">
                        <CrosshairCanvas
                          config={profile.config}
                          className="pointer-events-none block h-full w-full"
                        />
                      </div>

                      <button
                        type="button"
                        onClick={() => onStartRecording(profile.id)}
                        style={{
                          backgroundColor: isRecordingThis
                            ? "var(--accent)"
                            : "var(--surface-input)",
                          borderColor: isRecordingThis
                            ? "var(--accent)"
                            : "var(--border-subtle)",
                          color: isRecordingThis ? "var(--accent-fg)" : "var(--accent)",
                        }}
                        className={`min-w-[104px] rounded-lg border px-2.5 py-1.5 font-mono text-[11px] font-bold transition ${
                          isRecordingThis
                            ? "animate-pulse"
                            : "hover:border-[var(--accent)]"
                        }`}
                        title="Press any Key or Mouse Button (Esc to cancel, Backspace to clear)"
                      >
                        {isRecordingThis
                          ? "KEY / MOUSE..."
                          : (profile.hotkey ?? "UNBOUND")}
                      </button>

                      <div className="min-w-0 flex-1">
                        <input
                          type="text"
                          value={profile.name}
                          onChange={(e) => onRenameProfile(profile.id, e.target.value)}
                          aria-label={`Slot ${idx + 1} name`}
                          className="w-full min-w-0 truncate rounded bg-transparent px-1 py-0.5 font-bold outline-none focus:bg-black/25 focus:ring-1 focus:ring-[var(--accent)]"
                        />
                        <div className="px-1 text-[10px] text-[var(--muted-text)]">
                          {profile.config.hideBars
                            ? profile.config.hasCircle
                              ? "Circle + Dot"
                              : "Dot Only"
                            : `${profile.config.tShape ? "T-Shape" : "Cross"} · Len ${profile.config.length} · Gap ${profile.config.gap}`}
                          {profile.config.dynamicSpread ? " · Dynamic" : ""}
                        </div>
                      </div>
                    </div>

                    <div className="flex shrink-0 items-center gap-1.5">
                      <button
                        type="button"
                        onClick={() =>
                          void handleCopy(
                            encodeCrosshairCode(profile.config, profile.name),
                            profile.id,
                          )
                        }
                        style={{
                          backgroundColor: "var(--surface-subtle)",
                          borderColor: "var(--border-subtle)",
                        }}
                        className="rounded-lg border p-1.5 opacity-80 transition hover:border-[var(--accent)] hover:opacity-100"
                        title="Copy slot share code"
                      >
                        {copiedId === profile.id ? (
                          <Check className="h-3.5 w-3.5 text-emerald-400" />
                        ) : (
                          <Copy className="h-3.5 w-3.5" />
                        )}
                      </button>
                      <button
                        type="button"
                        onClick={() => onSelectProfile(profile.id)}
                        style={{
                          backgroundColor: isActive
                            ? "var(--accent)"
                            : "var(--surface-subtle)",
                          borderColor: isActive
                            ? "var(--accent)"
                            : "var(--border-subtle)",
                          color: isActive ? "var(--accent-fg)" : "var(--text-color)",
                        }}
                        className="rounded-lg border px-3 py-1.5 text-[11px] font-bold transition hover:border-[var(--accent)]"
                      >
                        {isActive ? "Active" : "Load"}
                      </button>
                      <button
                        type="button"
                        onClick={() => onSaveOverProfile(profile.id)}
                        style={{
                          backgroundColor: "var(--surface-subtle)",
                          borderColor: "var(--border-subtle)",
                        }}
                        className="rounded-lg border p-1.5 opacity-80 transition hover:border-[var(--accent)] hover:opacity-100"
                        title="Save current crosshair over this slot"
                      >
                        <Save className="h-3.5 w-3.5" />
                      </button>
                      <button
                        type="button"
                        onClick={() => onDeleteProfile(profile.id)}
                        className="rounded-lg border border-red-500/25 bg-red-500/10 p-1.5 text-red-400 transition hover:bg-red-500/25"
                        title="Delete slot"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  </div>

                  {/* Quick Mouse Button Helper Row when Recording */}
                  {isRecordingThis && (
                    <div
                      style={{
                        backgroundColor: "var(--surface-input)",
                        borderColor: "var(--border-subtle)",
                      }}
                      className="flex flex-wrap items-center gap-1.5 rounded-lg border p-2 text-[10px]"
                    >
                      <span className="flex items-center gap-1 font-semibold text-[var(--muted-text)]">
                        <Mouse className="h-3 w-3 text-[var(--accent)]" />
                        Quick Mouse Bind (or click Mouse4/5 / press any key):
                      </span>
                      {MOUSE_QUICK_BINDS.map((btnName) => (
                        <button
                          key={btnName}
                          type="button"
                          onMouseDown={(e) => e.stopPropagation()}
                          onClick={() => onManualBind(profile.id, btnName)}
                          style={{
                            backgroundColor: "var(--surface-subtle)",
                            borderColor: "var(--border-subtle)",
                          }}
                          className="rounded border px-2 py-0.5 font-mono font-semibold transition hover:border-[var(--accent)]"
                        >
                          {btnName}
                        </button>
                      ))}
                      <button
                        type="button"
                        onMouseDown={(e) => e.stopPropagation()}
                        onClick={() => onManualBind(profile.id, null)}
                        className="rounded border border-rose-500/30 bg-rose-500/15 px-2 py-0.5 font-semibold text-rose-300 transition hover:bg-rose-500/25"
                      >
                        Clear
                      </button>
                    </div>
                  )}
                </PanelCard>
              </motion.div>
            );
          })}
        </AnimatePresence>
      </div>

      {/* Crosshair Import / Export Studio */}
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
              Crosshair Import & Export
            </h3>
            <p className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              Share any crosshair as a compact Base64 string (
              <code className="font-mono">LUAIR-CH1:</code>) or a{" "}
              <code className="font-mono">.luair</code> /{" "}
              <code className="font-mono">.json</code> file.
            </p>
          </div>

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void handleCopy(currentShareCode, "current-code")}
              className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-[11px] font-bold text-[var(--accent-fg)] transition hover:opacity-90"
            >
              {copiedId === "current-code" ? (
                <Check className="h-3.5 w-3.5" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
              {copiedId === "current-code" ? "Code Copied!" : "Copy Base64 Code"}
            </button>
            <button
              type="button"
              onClick={() =>
                void handleCopy(
                  exportCrosshairJson(currentCrosshair, activeProfile?.name),
                  "current-json",
                )
              }
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
            >
              {copiedId === "current-json" ? (
                <Check className="h-3.5 w-3.5 text-emerald-400" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
              {copiedId === "current-json" ? "JSON Copied!" : "Copy JSON"}
            </button>
            <button
              type="button"
              onClick={handleDownloadFile}
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
            >
              <Download className="h-3.5 w-3.5" />
              Save .luair File
            </button>
          </div>
        </div>

        <div
          style={{
            backgroundColor: "var(--surface-input)",
            borderColor: "var(--border-subtle)",
          }}
          className="truncate rounded-lg border px-3 py-1.5 font-mono text-[10px] text-[var(--muted-text)]"
          title={currentShareCode}
        >
          {currentShareCode}
        </div>

        <div className="space-y-2">
          <textarea
            rows={2}
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder="Paste LUAIR-CH1:... Base64 code or .luair / .json contents here..."
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
              color: "var(--text-color)",
            }}
            className="luair-scroll w-full resize-none rounded-xl border p-2.5 font-mono text-[11px] outline-none focus:border-[var(--accent)]"
          />

          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => handleImportApply(false)}
                className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-[11px] font-bold text-[var(--accent-fg)] transition hover:opacity-90"
              >
                <Upload className="h-3.5 w-3.5" />
                Apply to Current
              </button>
              <button
                type="button"
                onClick={() => handleImportApply(true)}
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
              >
                <Plus className="h-3.5 w-3.5" />
                Import as New Slot
              </button>
              <input
                ref={fileInputRef}
                type="file"
                accept=".luair,.json,.txt"
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
                Load .luair / .json
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
