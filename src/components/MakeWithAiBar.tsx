import { useState, type FormEvent } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Check, Loader2, Sparkles, Wand2, X } from "lucide-react";
import { aiMakeCrosshair, aiMakeTheme } from "../lib/tauri";
import type { AiApplyResult } from "../types/ai";
import type { LuairState } from "../types/crosshair";

interface MakeWithAiBarProps {
  mode: "crosshair" | "theme";
  examples: string[];
  targetSlotId?: string | null;
  onApplied: (nextState: LuairState) => void;
}

export function MakeWithAiBar({
  mode,
  examples,
  targetSlotId,
  onApplied,
}: MakeWithAiBarProps) {
  const [open, setOpen] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<AiApplyResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e?: FormEvent, customPrompt?: string) => {
    e?.preventDefault();
    const text = (customPrompt ?? prompt).trim();
    if (!text || busy) return;

    setBusy(true);
    setError(null);
    setResult(null);

    try {
      const res =
        mode === "crosshair"
          ? await aiMakeCrosshair(text, targetSlotId)
          : await aiMakeTheme(text);
      setResult(res);
      onApplied(res.state);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex items-center gap-1.5 rounded-xl bg-gradient-to-r from-[var(--accent)] to-[var(--highlight)] px-3.5 py-2 text-[11px] font-black text-[var(--accent-fg)] shadow-[0_0_20px_var(--accent-glow)] transition hover:opacity-95"
        >
          <Sparkles className="h-3.5 w-3.5" />
          Make with AI
        </button>

        {result && (
          <div className="flex flex-wrap items-center gap-1.5 text-[10px]">
            <span className="flex items-center gap-1 rounded-full border border-emerald-400/30 bg-emerald-500/15 px-2.5 py-0.5 font-semibold text-emerald-400">
              <Check className="h-3 w-3" />
              {result.summary}
            </span>
            <span
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
              }}
              className="rounded-full border px-2 py-0.5 font-mono text-[9px] text-[var(--muted-text)]"
            >
              {result.engineUsed}
            </span>
          </div>
        )}
      </div>

      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.2 }}
            className="overflow-hidden"
          >
            <form
              onSubmit={(e) => void handleSubmit(e)}
              style={{
                backgroundColor: "var(--panel-color)",
                borderColor: "rgba(var(--accent-rgb), 0.4)",
                borderRadius: "calc(var(--radius) * 0.85)",
              }}
              className="space-y-2.5 border p-3.5"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="flex items-center gap-1.5 text-[11px] font-bold text-[var(--accent)]">
                  <Wand2 className="h-3.5 w-3.5" />
                  Describe the{" "}
                  {mode === "crosshair" ? "Crosshair, Preset & Keybind" : "Theme"} to
                  Generate
                </span>
                <button
                  type="button"
                  onClick={() => setOpen(false)}
                  className="rounded p-1 opacity-60 transition hover:opacity-100"
                  aria-label="Close AI prompt"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>

              <div className="flex gap-2">
                <input
                  type="text"
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  placeholder={examples[0]}
                  style={{
                    backgroundColor: "var(--surface-input)",
                    borderColor: "var(--border-subtle)",
                    color: "var(--text-color)",
                  }}
                  className="flex-1 rounded-xl border px-3 py-2 text-xs outline-none focus:border-[var(--accent)]"
                />
                <button
                  type="submit"
                  disabled={busy || !prompt.trim()}
                  className="flex items-center gap-1.5 rounded-xl bg-[var(--accent)] px-4 py-2 text-xs font-bold text-[var(--accent-fg)] transition hover:opacity-90 disabled:opacity-45"
                >
                  {busy ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Sparkles className="h-3.5 w-3.5" />
                  )}
                  {busy ? "Generating..." : "Apply"}
                </button>
              </div>

              <div className="flex flex-wrap gap-1.5">
                {examples.map((ex) => (
                  <button
                    key={ex}
                    type="button"
                    onClick={() => {
                      setPrompt(ex);
                      void handleSubmit(undefined, ex);
                    }}
                    style={{
                      backgroundColor: "var(--surface-subtle)",
                      borderColor: "var(--border-subtle)",
                    }}
                    className="rounded-lg border px-2 py-1 text-[10px] text-[var(--muted-text)] transition hover:border-[var(--accent)] hover:text-[var(--text-color)]"
                  >
                    {ex}
                  </button>
                ))}
              </div>

              {error && (
                <div className="rounded-lg border border-rose-500/30 bg-rose-500/15 px-3 py-1.5 text-[11px] font-semibold text-rose-300">
                  {error}
                </div>
              )}
            </form>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
