import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowRight,
  DownloadCloud,
  ExternalLink,
  RefreshCw,
  Sparkles,
  X,
} from "lucide-react";
import type { UpdateCheckResult, UpdateProgressEvent } from "../types/updater";

interface UpdateBannerProps {
  update: UpdateCheckResult;
  progress: UpdateProgressEvent | null;
  installing: boolean;
  error: string | null;
  onUpdateNow: () => void;
  onDismiss: () => void;
}

function renderReleaseLine(line: string, index: number) {
  const trimmed = line.trim();
  if (!trimmed) {
    return <div key={index} className="h-1.5" />;
  }
  if (trimmed.startsWith("### ") || trimmed.startsWith("## ") || trimmed.startsWith("# ")) {
    const heading = trimmed.replace(/^#{1,3}\s+/, "");
    return (
      <div
        key={index}
        className="pt-1.5 text-[11px] font-bold tracking-wide text-[var(--accent)]"
      >
        {heading}
      </div>
    );
  }
  if (trimmed.startsWith("- ") || trimmed.startsWith("* ")) {
    return (
      <div key={index} className="flex items-start gap-2 text-[11px] leading-relaxed">
        <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--accent)]" />
        <span>{trimmed.slice(2)}</span>
      </div>
    );
  }
  return (
    <div key={index} className="text-[11px] leading-relaxed opacity-90">
      {trimmed}
    </div>
  );
}

function formatMb(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function UpdateBanner({
  update,
  progress,
  installing,
  error,
  onUpdateNow,
  onDismiss,
}: UpdateBannerProps) {
  const lines = (update.releaseNotes ?? "").split(/\r?\n/);
  const percent = Math.max(0, Math.min(100, Math.round(progress?.percent ?? 0)));

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-sm"
      >
        <motion.div
          initial={{ opacity: 0, y: 16, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 10, scale: 0.97 }}
          transition={{ duration: 0.18, ease: "easeOut" }}
          style={{
            backgroundColor: "var(--panel-color)",
            borderColor: "var(--border-subtle)",
          }}
          className="w-full max-w-lg overflow-hidden rounded-2xl border p-5 shadow-2xl"
        >
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--accent)] text-[var(--accent-fg)] shadow-[0_0_18px_var(--accent-glow)]">
                <Sparkles className="h-5 w-5" />
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <span className="text-sm font-extrabold tracking-tight">
                    Luair Update Available
                  </span>
                  <span className="rounded-full border border-emerald-400/35 bg-emerald-500/15 px-2 py-0.5 text-[10px] font-bold text-emerald-300">
                    GitHub Release
                  </span>
                </div>
                <div className="mt-1 flex items-center gap-1.5 font-mono text-[11px]">
                  <span className="opacity-70">v{update.currentVersion}</span>
                  <ArrowRight className="h-3 w-3 text-[var(--accent)]" />
                  <span className="font-bold text-[var(--accent)]">
                    v{update.latestVersion}
                  </span>
                  {update.releaseDate && (
                    <span className="ml-1.5 text-[10px] opacity-55">
                      · {update.releaseDate.slice(0, 10)}
                    </span>
                  )}
                </div>
              </div>
            </div>

            {!installing && (
              <button
                type="button"
                onClick={onDismiss}
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="rounded-lg border p-1.5 opacity-75 transition hover:opacity-100"
                title="Dismiss"
              >
                <X className="h-4 w-4" />
              </button>
            )}
          </div>

          <div
            style={{
              backgroundColor: "var(--surface-input)",
              borderColor: "var(--border-subtle)",
            }}
            className="luair-scroll mt-4 max-h-48 space-y-1 overflow-y-auto rounded-xl border p-3.5"
          >
            <div className="mb-1 text-[10px] font-bold tracking-wider text-[var(--muted-text)] uppercase">
              Release Notes
            </div>
            {lines.map((line, i) => renderReleaseLine(line, i))}
          </div>

          {progress && installing && (
            <div className="mt-4 space-y-1.5">
              <div className="flex items-center justify-between text-[11px]">
                <span className="font-semibold text-[var(--accent)]">
                  {progress.message}
                </span>
                <span className="font-mono text-[10px] opacity-80">
                  {progress.totalBytes
                    ? `${formatMb(progress.downloadedBytes)} / ${formatMb(progress.totalBytes)} (${percent}%)`
                    : `${percent}%`}
                </span>
              </div>
              <div className="h-2 w-full overflow-hidden rounded-full bg-white/10">
                <motion.div
                  className="h-full bg-[var(--accent)] shadow-[0_0_12px_var(--accent-glow)]"
                  initial={{ width: 0 }}
                  animate={{ width: `${percent}%` }}
                  transition={{ duration: 0.15 }}
                />
              </div>
            </div>
          )}

          {error && (
            <div className="mt-3 rounded-xl border border-rose-500/35 bg-rose-500/12 px-3 py-2 text-[11px] text-rose-200">
              {error}
            </div>
          )}

          <div className="mt-4 flex flex-wrap items-center justify-between gap-2">
            <a
              href={update.releaseUrl}
              target="_blank"
              rel="noreferrer"
              className="flex items-center gap-1 text-[11px] font-semibold opacity-75 transition hover:text-[var(--accent)] hover:opacity-100"
            >
              <ExternalLink className="h-3.5 w-3.5" />
              View on GitHub
            </a>

            <div className="flex items-center gap-2">
              <button
                type="button"
                disabled={installing}
                onClick={onDismiss}
                style={{
                  backgroundColor: "var(--surface-input)",
                  borderColor: "var(--border-subtle)",
                }}
                className="rounded-xl border px-3.5 py-2 text-xs font-semibold transition hover:opacity-100 disabled:opacity-40"
              >
                Later
              </button>
              <button
                type="button"
                disabled={installing}
                onClick={onUpdateNow}
                className="flex items-center gap-2 rounded-xl bg-[var(--accent)] px-4 py-2 text-xs font-extrabold text-[var(--accent-fg)] shadow-[0_0_18px_var(--accent-glow)] transition hover:opacity-90 disabled:opacity-60"
              >
                {installing ? (
                  <RefreshCw className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <DownloadCloud className="h-3.5 w-3.5" />
                )}
                {installing ? "Updating & Restarting..." : "Update Now"}
              </button>
            </div>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
