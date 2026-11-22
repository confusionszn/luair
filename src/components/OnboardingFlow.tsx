import { useEffect, useState, type FormEvent } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  Cpu,
  Database,
  Eye,
  EyeOff,
  Keyboard,
  Lock,
  ShieldCheck,
  Sparkles,
  UserPlus,
  Zap,
} from "lucide-react";
import { CrosshairCanvas } from "./CrosshairCanvas";
import { LuairLogo } from "./LuairLogo";
import { PanelCard } from "./UiPrimitives";
import { completeOnboarding, signInLocal, signUpLocal } from "../lib/tauri";
import type { AuthSession } from "../types/auth";
import type { CrosshairProfile } from "../types/crosshair";
import { THEME_PRESETS, type ThemeConfig } from "../types/themes";

type OnboardingStage = "welcome" | "auth" | "wizard";
type AuthMode = "signup" | "signin";

interface OnboardingFlowProps {
  session: AuthSession;
  theme: ThemeConfig;
  profiles: CrosshairProfile[];
  currentHotkey: string;
  currentShield: boolean;
  onPreviewTheme: (theme: ThemeConfig) => void;
  onSessionChange: (session: AuthSession) => void;
}

const MISSION_PILLARS = [
  {
    icon: Lock,
    title: "Open-Source & 100% Local",
    desc: "Zero cloud servers, telemetry, or external license gates. Your vault stays encrypted on your own drive.",
  },
  {
    icon: Cpu,
    title: "Featherweight Performance",
    desc: "Native Rust core + zero-idle Canvas 2D renderer consumes 0% CPU when static with instant reactivity.",
  },
  {
    icon: ShieldCheck,
    title: "Undetectable Desktop Overlay",
    desc: "Independent click-through overlay with Win32 WDA_EXCLUDEFROMCAPTURE OBS/Discord cloak — never injects into games.",
  },
  {
    icon: Zap,
    title: "Pass-Through Keybinds",
    desc: "Switch weapon crosshair slots via keyboard or mouse buttons (Mouse4/5) with zero input delay.",
  },
];

const QUICK_HOTKEYS = ["F10", "Home", "Insert", "Ctrl+Shift+X"];

export function OnboardingFlow({
  session,
  theme,
  profiles,
  currentHotkey,
  currentShield,
  onPreviewTheme,
  onSessionChange,
}: OnboardingFlowProps) {
  const [stage, setStage] = useState<OnboardingStage>(() =>
    session.authenticated && session.account && !session.account.onboardingCompleted
      ? "wizard"
      : "welcome",
  );
  const [authMode, setAuthMode] = useState<AuthMode>(() =>
    session.hasExistingAccounts ? "signin" : "signup",
  );

  // Auth form fields
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [identifier, setIdentifier] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [rememberMe, setRememberMe] = useState(true);
  const [showPassword, setShowPassword] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // Setup wizard fields
  const [wizardStep, setWizardStep] = useState<0 | 1 | 2>(0);
  const [selectedTheme, setSelectedTheme] = useState<ThemeConfig>(theme);
  const [selectedProfileId, setSelectedProfileId] = useState<string>(
    profiles[0]?.id ?? "slot-1",
  );
  const [wizardShield, setWizardShield] = useState<boolean>(currentShield);
  const [wizardHotkey, setWizardHotkey] = useState<string>(currentHotkey || "F10");

  useEffect(() => {
    if (
      session.authenticated &&
      session.account &&
      !session.account.onboardingCompleted
    ) {
      setStage("wizard");
    } else if (!session.authenticated && stage === "wizard") {
      setStage("welcome");
    }
  }, [session.authenticated, session.account, stage]);

  const handleAuthSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setAuthError(null);
    setSubmitting(true);

    try {
      const nextSession =
        authMode === "signup"
          ? await signUpLocal({
              username,
              email,
              password,
              confirmPassword,
              rememberMe,
            })
          : await signInLocal({
              identifier,
              password,
              rememberMe,
            });
      setPassword("");
      setConfirmPassword("");
      onSessionChange(nextSession);
    } catch (err) {
      setAuthError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const handleFinishWizard = async (skip = false) => {
    setSubmitting(true);
    try {
      const nextSession = await completeOnboarding(
        skip
          ? {}
          : {
              starterProfileId: selectedProfileId,
              theme: selectedTheme,
              enableShield: wizardShield,
              menuHotkey: wizardHotkey,
            },
      );
      onSessionChange(nextSession);
    } finally {
      setSubmitting(false);
    }
  };

  const passwordStrength =
    password.length === 0
      ? 0
      : password.length < 8
        ? 1
        : /[A-Z]/.test(password) && /[0-9]/.test(password) && password.length >= 10
          ? 3
          : 2;

  return (
    <div className="luair-scroll flex min-h-0 flex-1 flex-col overflow-y-auto px-5 py-6 sm:px-8">
      <div className="mx-auto flex w-full max-w-3xl flex-1 flex-col justify-center">
        <AnimatePresence mode="wait">
          {stage === "welcome" && (
            <motion.div
              key="welcome"
              initial={{ opacity: 0, y: 14 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -12 }}
              transition={{ duration: 0.25, ease: "easeOut" }}
              className="space-y-6"
            >
              <div className="flex flex-col items-center text-center">
                <motion.div
                  animate={{ scale: [1, 1.04, 1] }}
                  transition={{ duration: 3.4, repeat: Infinity, ease: "easeInOut" }}
                >
                  <LuairLogo size={68} />
                </motion.div>

                <span
                  style={{
                    backgroundColor: "rgba(var(--accent-rgb), 0.14)",
                    borderColor: "rgba(var(--accent-rgb), 0.35)",
                  }}
                  className="mt-4 inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-[10px] font-bold tracking-widest text-[var(--accent)] uppercase"
                >
                  <Sparkles className="h-3 w-3" />
                  Independent Desktop Precision
                </span>

                <h1 className="mt-3 text-2xl font-black tracking-tight sm:text-3xl">
                  Welcome to{" "}
                  <span className="bg-gradient-to-r from-[var(--accent)] to-[var(--highlight)] bg-clip-text text-transparent">
                    Luair
                  </span>
                </h1>
                <p className="mt-2 max-w-xl text-xs leading-relaxed text-[var(--muted-text)] sm:text-sm">
                  An open-source, lightweight, undetectable desktop crosshair overlay
                  engineered for competitive clarity without process injection, cloud
                  accounts, or input latency.
                </p>
              </div>

              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                {MISSION_PILLARS.map((item, index) => {
                  const Icon = item.icon;
                  return (
                    <motion.div
                      key={item.title}
                      initial={{ opacity: 0, y: 12 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: 0.08 + index * 0.06, duration: 0.24 }}
                    >
                      <PanelCard
                        surfaceMode={theme.surfaceMode}
                        className="h-full space-y-1.5 p-4"
                      >
                        <div className="flex items-center gap-2">
                          <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-[var(--accent)]/15 text-[var(--accent)]">
                            <Icon className="h-4 w-4" />
                          </div>
                          <h2 className="text-xs font-bold">{item.title}</h2>
                        </div>
                        <p className="text-[11px] leading-relaxed text-[var(--muted-text)]">
                          {item.desc}
                        </p>
                      </PanelCard>
                    </motion.div>
                  );
                })}
              </div>

              <div className="flex flex-wrap items-center justify-center gap-3 pt-1">
                <button
                  type="button"
                  onClick={() => {
                    setAuthMode("signup");
                    setAuthError(null);
                    setStage("auth");
                  }}
                  className="flex items-center gap-2 rounded-xl bg-[var(--accent)] px-6 py-3 text-xs font-black tracking-wide text-[var(--accent-fg)] shadow-[0_0_24px_var(--accent-glow)] transition hover:opacity-95"
                >
                  Get Started
                  <ArrowRight className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setAuthMode("signin");
                    setAuthError(null);
                    setStage("auth");
                  }}
                  style={{
                    backgroundColor: "var(--surface-input)",
                    borderColor: "var(--border-subtle)",
                  }}
                  className="flex items-center gap-2 rounded-xl border px-5 py-3 text-xs font-bold transition hover:border-[var(--accent)]"
                >
                  Sign In to Local Vault
                </button>
              </div>
            </motion.div>
          )}

          {stage === "auth" && (
            <motion.div
              key="auth"
              initial={{ opacity: 0, y: 14 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -12 }}
              transition={{ duration: 0.22 }}
              className="mx-auto w-full max-w-md space-y-4"
            >
              <button
                type="button"
                onClick={() => setStage("welcome")}
                className="inline-flex items-center gap-1.5 text-[11px] font-semibold text-[var(--muted-text)] transition hover:text-[var(--text-color)]"
              >
                <ArrowLeft className="h-3.5 w-3.5" />
                Back to Welcome
              </button>

              <PanelCard surfaceMode={theme.surfaceMode} className="space-y-4 p-5">
                <div className="flex items-center justify-between gap-2">
                  <div>
                    <h2 className="text-base font-black">
                      {authMode === "signup"
                        ? "Create Local Account"
                        : "Unlock Local Vault"}
                    </h2>
                    <p className="mt-0.5 flex items-center gap-1.5 text-[10px] text-[var(--muted-text)]">
                      <Database className="h-3 w-3 text-[var(--accent)]" />
                      {session.vaultEngine}
                    </p>
                  </div>
                </div>

                {/* Sign Up / Sign In Switcher */}
                <div
                  style={{
                    backgroundColor: "var(--surface-input)",
                    borderColor: "var(--border-subtle)",
                  }}
                  className="relative grid grid-cols-2 rounded-xl border p-1"
                >
                  {(
                    [
                      { id: "signup", label: "Sign Up" },
                      { id: "signin", label: "Sign In" },
                    ] as const
                  ).map((item) => {
                    const active = authMode === item.id;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => {
                          setAuthMode(item.id);
                          setAuthError(null);
                        }}
                        style={{
                          color: active ? "var(--accent-fg)" : "var(--text-color)",
                        }}
                        className="relative z-10 rounded-lg py-1.5 text-xs font-bold transition-colors"
                      >
                        {active && (
                          <motion.span
                            layoutId="auth-mode-pill"
                            transition={{ type: "spring", stiffness: 420, damping: 32 }}
                            className="absolute inset-0 -z-10 rounded-lg bg-[var(--accent)] shadow-[0_0_14px_var(--accent-glow)]"
                          />
                        )}
                        {item.label}
                      </button>
                    );
                  })}
                </div>

                <form onSubmit={(e) => void handleAuthSubmit(e)} className="space-y-3 text-xs">
                  {authMode === "signup" ? (
                    <>
                      <label className="block space-y-1">
                        <span className="text-[11px] font-semibold text-[var(--muted-text)]">
                          Username
                        </span>
                        <input
                          type="text"
                          required
                          value={username}
                          onChange={(e) => setUsername(e.target.value)}
                          placeholder="e.g., TacticalFox"
                          style={{
                            backgroundColor: "var(--surface-input)",
                            borderColor: "var(--border-subtle)",
                            color: "var(--text-color)",
                          }}
                          className="w-full rounded-xl border px-3 py-2 outline-none focus:border-[var(--accent)]"
                        />
                      </label>

                      <label className="block space-y-1">
                        <span className="text-[11px] font-semibold text-[var(--muted-text)]">
                          Email
                        </span>
                        <input
                          type="email"
                          required
                          value={email}
                          onChange={(e) => setEmail(e.target.value)}
                          placeholder="you@local.dev"
                          style={{
                            backgroundColor: "var(--surface-input)",
                            borderColor: "var(--border-subtle)",
                            color: "var(--text-color)",
                          }}
                          className="w-full rounded-xl border px-3 py-2 outline-none focus:border-[var(--accent)]"
                        />
                      </label>
                    </>
                  ) : (
                    <label className="block space-y-1">
                      <span className="text-[11px] font-semibold text-[var(--muted-text)]">
                        Username or Email
                      </span>
                      <input
                        type="text"
                        required
                        value={identifier}
                        onChange={(e) => setIdentifier(e.target.value)}
                        placeholder="Username or email"
                        style={{
                          backgroundColor: "var(--surface-input)",
                          borderColor: "var(--border-subtle)",
                          color: "var(--text-color)",
                        }}
                        className="w-full rounded-xl border px-3 py-2 outline-none focus:border-[var(--accent)]"
                      />
                    </label>
                  )}

                  <label className="block space-y-1">
                    <span className="text-[11px] font-semibold text-[var(--muted-text)]">
                      Password
                    </span>
                    <div className="relative">
                      <input
                        type={showPassword ? "text" : "password"}
                        required
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        placeholder={
                          authMode === "signup"
                            ? "Minimum 8 characters"
                            : "Enter password"
                        }
                        style={{
                          backgroundColor: "var(--surface-input)",
                          borderColor: "var(--border-subtle)",
                          color: "var(--text-color)",
                        }}
                        className="w-full rounded-xl border py-2 pr-9 pl-3 outline-none focus:border-[var(--accent)]"
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword((v) => !v)}
                        className="absolute top-1/2 right-2.5 -translate-y-1/2 opacity-60 transition hover:opacity-100"
                        aria-label={showPassword ? "Hide password" : "Show password"}
                      >
                        {showPassword ? (
                          <EyeOff className="h-4 w-4" />
                        ) : (
                          <Eye className="h-4 w-4" />
                        )}
                      </button>
                    </div>
                  </label>

                  {authMode === "signup" && (
                    <>
                      {/* Strength bar */}
                      <div className="flex items-center gap-1.5">
                        {[1, 2, 3].map((lvl) => (
                          <div
                            key={lvl}
                            className="h-1 flex-1 rounded-full transition-colors"
                            style={{
                              backgroundColor:
                                passwordStrength >= lvl
                                  ? passwordStrength === 3
                                    ? "#34d399"
                                    : passwordStrength === 2
                                      ? "var(--accent)"
                                      : "#fb7185"
                                  : "var(--border-subtle)",
                            }}
                          />
                        ))}
                        <span className="text-[10px] text-[var(--muted-text)]">
                          {passwordStrength === 0
                            ? "8+ chars"
                            : passwordStrength === 1
                              ? "Too short"
                              : passwordStrength === 2
                                ? "Good"
                                : "Strong"}
                        </span>
                      </div>

                      <label className="block space-y-1">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-semibold text-[var(--muted-text)]">
                            Confirm Password
                          </span>
                          {confirmPassword.length > 0 && (
                            <span
                              className={`text-[10px] font-semibold ${
                                password === confirmPassword
                                  ? "text-emerald-400"
                                  : "text-rose-400"
                              }`}
                            >
                              {password === confirmPassword ? "Matches" : "Mismatch"}
                            </span>
                          )}
                        </div>
                        <input
                          type={showPassword ? "text" : "password"}
                          required
                          value={confirmPassword}
                          onChange={(e) => setConfirmPassword(e.target.value)}
                          placeholder="Re-enter password"
                          style={{
                            backgroundColor: "var(--surface-input)",
                            borderColor: "var(--border-subtle)",
                            color: "var(--text-color)",
                          }}
                          className="w-full rounded-xl border px-3 py-2 outline-none focus:border-[var(--accent)]"
                        />
                      </label>
                    </>
                  )}

                  <label className="flex cursor-pointer items-center gap-2 pt-0.5 text-[11px]">
                    <input
                      type="checkbox"
                      checked={rememberMe}
                      onChange={(e) => setRememberMe(e.target.checked)}
                    />
                    <span>Remember login across app restarts</span>
                  </label>

                  {authError && (
                    <div className="rounded-xl border border-rose-500/30 bg-rose-500/15 px-3 py-2 text-[11px] font-semibold text-rose-300">
                      {authError}
                    </div>
                  )}

                  <button
                    type="submit"
                    disabled={submitting}
                    className="flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] py-2.5 text-xs font-black text-[var(--accent-fg)] shadow-[0_0_20px_var(--accent-glow)] transition hover:opacity-95 disabled:opacity-50"
                  >
                    <UserPlus className="h-4 w-4" />
                    {submitting
                      ? "Securing Local Vault..."
                      : authMode === "signup"
                        ? "Create Local Profile & Continue"
                        : "Sign In"}
                  </button>
                </form>
              </PanelCard>
            </motion.div>
          )}

          {stage === "wizard" && (
            <motion.div
              key="wizard"
              initial={{ opacity: 0, y: 14 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -12 }}
              transition={{ duration: 0.22 }}
              className="space-y-4"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <span className="text-[10px] font-bold tracking-widest text-[var(--accent)] uppercase">
                    Setup Wizard · Step {wizardStep + 1} of 3
                  </span>
                  <h2 className="text-lg font-black">
                    Welcome, @{session.account?.username}
                  </h2>
                </div>
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => void handleFinishWizard(true)}
                  style={{
                    backgroundColor: "var(--surface-input)",
                    borderColor: "var(--border-subtle)",
                  }}
                  className="rounded-lg border px-3 py-1.5 text-[11px] font-semibold transition hover:border-[var(--accent)]"
                >
                  Skip to Studio
                </button>
              </div>

              {/* Progress Steps */}
              <div className="grid grid-cols-3 gap-2">
                {["Visual Theme", "Starter Crosshair", "OBS Shield & Hotkey"].map(
                  (label, i) => (
                    <button
                      key={label}
                      type="button"
                      onClick={() => setWizardStep(i as 0 | 1 | 2)}
                      className="space-y-1 text-left"
                    >
                      <div
                        className="h-1.5 rounded-full transition-colors"
                        style={{
                          backgroundColor:
                            i <= wizardStep ? "var(--accent)" : "var(--border-subtle)",
                        }}
                      />
                      <span
                        className={`block text-[10px] font-bold ${
                          i === wizardStep
                            ? "text-[var(--accent)]"
                            : "text-[var(--muted-text)]"
                        }`}
                      >
                        {i + 1}. {label}
                      </span>
                    </button>
                  ),
                )}
              </div>

              <PanelCard surfaceMode={theme.surfaceMode} className="space-y-4 p-5">
                {wizardStep === 0 && (
                  <div className="space-y-3">
                    <div>
                      <h3 className="text-xs font-bold">Choose Your Aesthetic</h3>
                      <p className="text-[11px] text-[var(--muted-text)]">
                        Click any preset to preview Liquid Glass or Solid Minimalist live.
                      </p>
                    </div>
                    <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-4">
                      {THEME_PRESETS.map((preset) => {
                        const active = selectedTheme.id === preset.id;
                        return (
                          <button
                            key={preset.id}
                            type="button"
                            onClick={() => {
                              setSelectedTheme(preset);
                              onPreviewTheme(preset);
                            }}
                            style={{
                              backgroundColor: preset.background,
                              borderColor: active ? preset.accent : "var(--border-subtle)",
                              color: preset.text,
                              boxShadow: active ? `0 0 18px ${preset.accent}44` : undefined,
                            }}
                            className="flex flex-col justify-between rounded-xl border p-3 text-left transition hover:-translate-y-0.5"
                          >
                            <div className="flex items-center justify-between">
                              <div className="flex items-center gap-1">
                                <span
                                  className="h-2.5 w-2.5 rounded-full"
                                  style={{ backgroundColor: preset.accent }}
                                />
                                <span
                                  className="h-2.5 w-2.5 rounded-full"
                                  style={{ backgroundColor: preset.highlight }}
                                />
                              </div>
                              <span
                                className="rounded px-1.5 py-0.5 text-[9px] font-bold uppercase"
                                style={{
                                  backgroundColor: preset.surface,
                                  color: preset.mutedText,
                                }}
                              >
                                {preset.surfaceMode === "liquid-glass" ? "Glass" : "Solid"}
                              </span>
                            </div>
                            <div className="mt-3 flex items-center justify-between text-[11px] font-bold">
                              <span>{preset.name}</span>
                              {active && <Check className="h-3.5 w-3.5" />}
                            </div>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                )}

                {wizardStep === 1 && (
                  <div className="space-y-3">
                    <div>
                      <h3 className="text-xs font-bold">Select Starter Crosshair</h3>
                      <p className="text-[11px] text-[var(--muted-text)]">
                        All 4 slots remain bound to your non-blocking weapon keys.
                      </p>
                    </div>
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      {profiles.map((p) => {
                        const active = selectedProfileId === p.id;
                        return (
                          <button
                            key={p.id}
                            type="button"
                            onClick={() => setSelectedProfileId(p.id)}
                            style={{
                              backgroundColor: "var(--surface-input)",
                              borderColor: active ? "var(--accent)" : "var(--border-subtle)",
                            }}
                            className={`flex items-center gap-3 rounded-xl border p-3 text-left transition ${
                              active ? "ring-1 ring-[var(--accent)]" : ""
                            }`}
                          >
                            <div className="h-12 w-12 shrink-0 overflow-hidden rounded-lg border border-white/15 bg-[#06080f]">
                              <CrosshairCanvas
                                config={p.config}
                                className="pointer-events-none block h-full w-full"
                              />
                            </div>
                            <div className="min-w-0 flex-1">
                              <div className="truncate text-xs font-bold">{p.name}</div>
                              <div className="mt-0.5 font-mono text-[10px] text-[var(--accent)]">
                                Hotkey: {p.hotkey ?? "Unbound"}
                              </div>
                            </div>
                            {active && <Check className="h-4 w-4 text-[var(--accent)]" />}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                )}

                {wizardStep === 2 && (
                  <div className="space-y-4">
                    <label
                      style={{
                        backgroundColor: "var(--surface-input)",
                        borderColor: "var(--border-subtle)",
                      }}
                      className="flex cursor-pointer items-center justify-between gap-4 rounded-xl border p-3.5"
                    >
                      <div className="flex items-start gap-3">
                        <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-[var(--accent)]" />
                        <div>
                          <div className="text-xs font-bold">
                            Enable OBS / Discord Capture Cloak
                          </div>
                          <div className="mt-0.5 text-[10px] text-[var(--muted-text)]">
                            Applies WDA_EXCLUDEFROMCAPTURE so streams & screenshots stay
                            clean.
                          </div>
                        </div>
                      </div>
                      <input
                        type="checkbox"
                        checked={wizardShield}
                        onChange={(e) => setWizardShield(e.target.checked)}
                      />
                    </label>

                    <div className="space-y-2">
                      <div className="flex items-center gap-1.5 text-xs font-bold">
                        <Keyboard className="h-4 w-4 text-[var(--accent)]" />
                        Menu Toggle Hotkey
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {QUICK_HOTKEYS.map((hk) => {
                          const active = wizardHotkey === hk;
                          return (
                            <button
                              key={hk}
                              type="button"
                              onClick={() => setWizardHotkey(hk)}
                              style={{
                                backgroundColor: active
                                  ? "var(--accent)"
                                  : "var(--surface-input)",
                                borderColor: active
                                  ? "var(--accent)"
                                  : "var(--border-subtle)",
                                color: active ? "var(--accent-fg)" : "var(--text-color)",
                              }}
                              className="rounded-lg border px-3 py-1.5 font-mono text-xs font-bold transition"
                            >
                              {hk}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  </div>
                )}

                <div
                  style={{ borderColor: "var(--border-subtle)" }}
                  className="flex items-center justify-between border-t pt-4"
                >
                  <button
                    type="button"
                    disabled={wizardStep === 0}
                    onClick={() =>
                      setWizardStep((s) => (s > 0 ? ((s - 1) as 0 | 1 | 2) : s))
                    }
                    style={{
                      backgroundColor: "var(--surface-input)",
                      borderColor: "var(--border-subtle)",
                    }}
                    className="flex items-center gap-1.5 rounded-lg border px-3.5 py-2 text-xs font-semibold transition disabled:opacity-35"
                  >
                    <ArrowLeft className="h-3.5 w-3.5" />
                    Back
                  </button>

                  {wizardStep < 2 ? (
                    <button
                      type="button"
                      onClick={() =>
                        setWizardStep((s) => (s < 2 ? ((s + 1) as 0 | 1 | 2) : s))
                      }
                      className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-4 py-2 text-xs font-bold text-[var(--accent-fg)] transition hover:opacity-90"
                    >
                      Continue
                      <ArrowRight className="h-3.5 w-3.5" />
                    </button>
                  ) : (
                    <button
                      type="button"
                      disabled={submitting}
                      onClick={() => void handleFinishWizard(false)}
                      className="flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-5 py-2 text-xs font-black text-[var(--accent-fg)] shadow-[0_0_20px_var(--accent-glow)] transition hover:opacity-95 disabled:opacity-50"
                    >
                      Launch Luair Studio
                      <Sparkles className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
              </PanelCard>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
