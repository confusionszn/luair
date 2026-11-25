import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  Bot,
  Check,
  Cpu,
  Download,
  Loader2,
  Play,
  Send,
  Sparkles,
  Square,
  Trash2,
  X,
} from "lucide-react";
import {
  aiChat,
  cancelAiDownload,
  deleteAiModel,
  downloadAiModel,
  getAiStatus,
  loadAiModel,
  onAiDownloadProgress,
  unloadAiModel,
} from "../lib/tauri";
import type {
  AiDownloadProgress,
  AiEngineStatus,
  ChatMessage,
} from "../types/ai";
import type { LuairState } from "../types/crosshair";
import type { ThemeSurfaceMode } from "../types/themes";
import { PanelCard } from "./UiPrimitives";

interface AiAssistantTabProps {
  surfaceMode: ThemeSurfaceMode;
  onStateApplied: (nextState: LuairState) => void;
}

interface UiChatEntry {
  id: string;
  role: "user" | "assistant";
  content: string;
  engineUsed?: string;
  appliedAction?: string | null;
}

const STARTER_PROMPTS = [
  "Create a new preset called Sniper with a crimson dot on key 5",
  "Put a cyan T-shape crosshair on preset 2",
  "Make a neon purple liquid glass theme",
  "Enable OBS Shield",
];

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 MB";
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function AiAssistantTab({
  surfaceMode,
  onStateApplied,
}: AiAssistantTabProps) {
  const [status, setStatus] = useState<AiEngineStatus | null>(null);
  const [progress, setProgress] = useState<AiDownloadProgress | null>(null);
  const [loadingModelId, setLoadingModelId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [messages, setMessages] = useState<UiChatEntry[]>([
    {
      id: "welcome-ai",
      role: "assistant",
      content:
        "Welcome to Luair AI Assistant! Download Luair Candle (475 MB) with one click for 100% private local chat — or ask me right now to create presets, put crosshairs on slots, design themes, or toggle OBS Shield.",
      engineUsed: "Local AI Hub",
    },
  ]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  const refreshStatus = async () => {
    try {
      const next = await getAiStatus();
      setStatus(next);
    } catch {
      // Ignore status refresh failure.
    }
  };

  useEffect(() => {
    void refreshStatus();
    let unlisten: (() => void) | undefined;
    void onAiDownloadProgress((p) => {
      setProgress(p);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, [messages, sending]);

  const handleDownload = async (modelId: string) => {
    setError(null);
    setProgress({
      modelId,
      stage: "connecting",
      downloadedBytes: 0,
      totalBytes: null,
      percent: 0,
      message: "Connecting to Luair Candle download...",
    });
    try {
      const next = await downloadAiModel(modelId);
      setStatus(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      await refreshStatus();
    } finally {
      setProgress(null);
    }
  };

  const handleLoad = async (modelId: string) => {
    setError(null);
    setLoadingModelId(modelId);
    try {
      const next = await loadAiModel(modelId);
      setStatus(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingModelId(null);
    }
  };

  const handleUnload = async () => {
    setError(null);
    const next = await unloadAiModel();
    setStatus(next);
  };

  const handleDelete = async (modelId: string) => {
    setError(null);
    const next = await deleteAiModel(modelId);
    setStatus(next);
  };

  const handleSend = async (e?: FormEvent, overridePrompt?: string) => {
    e?.preventDefault();
    const promptText = (overridePrompt ?? input).trim();
    if (!promptText || sending) return;

    const history: ChatMessage[] = messages
      .filter((m) => m.id !== "welcome-ai")
      .slice(-6)
      .map((m) => ({ role: m.role, content: m.content }));

    const userMsg: UiChatEntry = {
      id: `u-${Date.now()}`,
      role: "user",
      content: promptText,
    };
    setMessages((prev) => [...prev, userMsg]);
    if (!overridePrompt) setInput("");
    setSending(true);
    setError(null);

    try {
      const resp = await aiChat(history, promptText);
      if (resp.updatedState) {
        onStateApplied(resp.updatedState);
      }
      setMessages((prev) => [
        ...prev,
        {
          id: `a-${Date.now()}`,
          role: "assistant",
          content: resp.reply,
          engineUsed: resp.engineUsed,
          appliedAction: resp.appliedAction,
        },
      ]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Luair Candle Local AI Engine */}
      <PanelCard surfaceMode={surfaceMode} className="space-y-3 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h2 className="flex items-center gap-1.5 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
              <Cpu className="h-3.5 w-3.5" />
              Luair Candle · Embedded Local AI (No API Keys)
            </h2>
            <p className="mt-0.5 text-[10px] text-[var(--muted-text)]">
              Active backend:{" "}
              <strong className="text-[var(--text-color)]">
                {status?.activeBackend ?? "Initializing..."}
              </strong>
            </p>
          </div>
        </div>

        {(status?.models ?? []).slice(0, 1).map((model) => {
          const isDownloading =
            progress?.modelId === model.id ||
            status?.downloadingModelId === model.id;
          const isLoadingThis = loadingModelId === model.id;

          return (
            <div
              key={model.id}
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: model.loaded ? "var(--accent)" : "var(--border-subtle)",
              }}
              className="flex flex-col justify-between gap-3 rounded-xl border p-4"
            >
              <div>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="text-sm font-bold">{model.name}</span>
                  <span
                    style={{
                      backgroundColor: model.loaded
                        ? "rgba(var(--accent-rgb), 0.16)"
                        : "var(--surface-subtle)",
                    }}
                    className={`rounded-full px-2.5 py-0.5 font-mono text-[10px] font-bold uppercase ${
                      model.loaded
                        ? "text-[var(--accent)]"
                        : "text-[var(--muted-text)]"
                    }`}
                  >
                    {model.quantization} · {model.approxSizeMb} MB
                  </span>
                </div>
                <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--muted-text)]">
                  {model.description}
                </p>
              </div>

              {isDownloading && progress ? (
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-[10px]">
                    <span className="font-semibold text-[var(--accent)]">
                      {progress.message}
                    </span>
                    <span className="font-mono">
                      {formatBytes(progress.downloadedBytes)}
                      {progress.totalBytes
                        ? ` / ${formatBytes(progress.totalBytes)}`
                        : ""}{" "}
                      ({progress.percent.toFixed(0)}%)
                    </span>
                  </div>
                  <div
                    className="h-1.5 w-full overflow-hidden rounded-full"
                    style={{ backgroundColor: "var(--border-subtle)" }}
                  >
                    <div
                      className="h-full rounded-full bg-[var(--accent)] transition-all duration-150"
                      style={{ width: `${Math.max(4, progress.percent)}%` }}
                    />
                  </div>
                  <button
                    type="button"
                    onClick={() => void cancelAiDownload()}
                    className="flex items-center gap-1 text-[10px] font-semibold text-rose-400 hover:underline"
                  >
                    <X className="h-3 w-3" />
                    Cancel Download
                  </button>
                </div>
              ) : (
                <div className="flex flex-wrap items-center justify-between gap-2">
                  {!model.downloaded ? (
                    <button
                      type="button"
                      disabled={Boolean(status?.downloadingModelId)}
                      onClick={() => void handleDownload(model.id)}
                      className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3.5 py-2 text-[11px] font-bold text-[var(--accent-fg)] transition hover:opacity-90 disabled:opacity-45"
                    >
                      <Download className="h-3.5 w-3.5" />
                      Download Luair Candle ({model.approxSizeMb} MB)
                    </button>
                  ) : (
                    <div className="flex items-center gap-1.5">
                      {model.loaded ? (
                        <button
                          type="button"
                          onClick={() => void handleUnload()}
                          style={{
                            backgroundColor: "var(--surface-subtle)",
                            borderColor: "var(--border-subtle)",
                          }}
                          className="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[11px] font-bold transition hover:border-[var(--accent)]"
                        >
                          <Square className="h-3 w-3" />
                          Unload
                        </button>
                      ) : (
                        <button
                          type="button"
                          disabled={Boolean(loadingModelId)}
                          onClick={() => void handleLoad(model.id)}
                          className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-[11px] font-bold text-[var(--accent-fg)] transition hover:opacity-90 disabled:opacity-45"
                        >
                          {isLoadingThis ? (
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                          ) : (
                            <Play className="h-3.5 w-3.5" />
                          )}
                          {isLoadingThis ? "Loading..." : "Load Engine"}
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => void handleDelete(model.id)}
                        className="rounded-lg border border-rose-500/25 bg-rose-500/10 p-1.5 text-rose-400 transition hover:bg-rose-500/20"
                        title="Delete local Luair Candle weights"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  )}

                  {model.downloaded && (
                    <span className="font-mono text-[10px] text-emerald-400">
                      Ready ({formatBytes(model.localSizeBytes)})
                    </span>
                  )}
                </div>
              )}
            </div>
          );
        })}

        {error && (
          <div className="rounded-xl border border-rose-500/30 bg-rose-500/15 px-3 py-2 text-[11px] font-semibold text-rose-300">
            {error}
          </div>
        )}
      </PanelCard>

      {/* Luair AI Chat Console */}
      <PanelCard surfaceMode={surfaceMode} className="flex h-[340px] flex-col overflow-hidden">
        <div
          style={{ borderColor: "var(--border-subtle)" }}
          className="flex items-center justify-between border-b px-4 py-2.5"
        >
          <span className="flex items-center gap-1.5 text-[11px] font-bold">
            <Bot className="h-4 w-4 text-[var(--accent)]" />
            Luair AI Assistant
          </span>
          <span className="text-[10px] text-[var(--muted-text)]">
            100% Local Inference
          </span>
        </div>

        <div
          ref={scrollRef}
          className="luair-scroll flex-1 space-y-3 overflow-y-auto p-4 text-xs"
        >
          {messages.map((msg) => {
            const isUser = msg.role === "user";
            return (
              <div
                key={msg.id}
                className={`flex flex-col ${isUser ? "items-end" : "items-start"}`}
              >
                <div
                  style={{
                    backgroundColor: isUser ? "var(--accent)" : "var(--surface-input)",
                    borderColor: isUser ? "var(--accent)" : "var(--border-subtle)",
                    color: isUser ? "var(--accent-fg)" : "var(--text-color)",
                  }}
                  className="max-w-[85%] rounded-2xl border px-3.5 py-2.5 leading-relaxed"
                >
                  <div>{msg.content}</div>
                  {msg.appliedAction && (
                    <div className="mt-2 inline-flex items-center gap-1 rounded-full border border-emerald-400/35 bg-emerald-500/15 px-2.5 py-0.5 text-[10px] font-bold text-emerald-400">
                      <Check className="h-3 w-3" />
                      {msg.appliedAction}
                    </div>
                  )}
                </div>
                {msg.engineUsed && (
                  <span className="mt-1 px-1 font-mono text-[9px] text-[var(--muted-text)]">
                    {msg.engineUsed}
                  </span>
                )}
              </div>
            );
          })}

          {sending && (
            <div className="flex items-center gap-2 text-[11px] text-[var(--muted-text)]">
              <Loader2 className="h-3.5 w-3.5 animate-spin text-[var(--accent)]" />
              Thinking locally...
            </div>
          )}
        </div>

        <div
          style={{ borderColor: "var(--border-subtle)" }}
          className="space-y-2 border-t p-3"
        >
          <div className="flex flex-wrap gap-1.5">
            {STARTER_PROMPTS.map((p) => (
              <button
                key={p}
                type="button"
                onClick={() => void handleSend(undefined, p)}
                style={{
                  backgroundColor: "var(--surface-subtle)",
                  borderColor: "var(--border-subtle)",
                }}
                className="flex items-center gap-1 rounded-lg border px-2 py-1 text-[10px] text-[var(--muted-text)] transition hover:border-[var(--accent)] hover:text-[var(--text-color)]"
              >
                <Sparkles className="h-2.5 w-2.5 text-[var(--accent)]" />
                {p}
              </button>
            ))}
          </div>

          <form onSubmit={(e) => void handleSend(e)} className="flex gap-2">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Ask Luair AI or request a crosshair, theme, or OBS shield change..."
              style={{
                backgroundColor: "var(--surface-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-color)",
              }}
              className="flex-1 rounded-xl border px-3 py-2 text-xs outline-none focus:border-[var(--accent)]"
            />
            <button
              type="submit"
              disabled={sending || !input.trim()}
              className="flex items-center gap-1.5 rounded-xl bg-[var(--accent)] px-4 py-2 text-xs font-bold text-[var(--accent-fg)] transition hover:opacity-90 disabled:opacity-45"
            >
              <Send className="h-3.5 w-3.5" />
              Send
            </button>
          </form>
        </div>
      </PanelCard>
    </div>
  );
}
