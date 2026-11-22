import { useId } from "react";

interface LuairLogoProps {
  size?: number;
  showWordmark?: boolean;
  subtitle?: string;
  className?: string;
}

export function LuairLogo({
  size = 28,
  showWordmark = false,
  subtitle,
  className = "",
}: LuairLogoProps) {
  const gradId = useId();
  const softId = useId();

  return (
    <div className={`inline-flex items-center gap-2.5 ${className}`}>
      <svg
        width={size}
        height={size}
        viewBox="0 0 512 512"
        fill="none"
        className="shrink-0 drop-shadow-[0_0_12px_var(--accent-glow)]"
        aria-hidden="true"
      >
        <defs>
          <linearGradient id={gradId} x1="120" y1="112" x2="392" y2="400" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="var(--accent)" />
            <stop offset="100%" stopColor="var(--highlight)" />
          </linearGradient>
          <linearGradient id={softId} x1="392" y1="112" x2="120" y2="400" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.92" />
            <stop offset="100%" stopColor="var(--highlight)" stopOpacity="0.52" />
          </linearGradient>
        </defs>

        <rect
          x="24"
          y="24"
          width="464"
          height="464"
          rx="116"
          fill="var(--panel-color)"
          fillOpacity="0.9"
          stroke={`url(#${gradId})`}
          strokeOpacity="0.55"
          strokeWidth="12"
        />

        <path
          d="M 136 176 A 144 144 0 0 1 176 136"
          stroke={`url(#${gradId})`}
          strokeWidth="12"
          strokeLinecap="round"
          opacity="0.6"
        />
        <path
          d="M 336 136 A 144 144 0 0 1 376 176"
          stroke={`url(#${gradId})`}
          strokeWidth="12"
          strokeLinecap="round"
          opacity="0.6"
        />
        <path
          d="M 376 336 A 144 144 0 0 1 336 376"
          stroke={`url(#${gradId})`}
          strokeWidth="12"
          strokeLinecap="round"
          opacity="0.6"
        />
        <path
          d="M 176 376 A 144 144 0 0 1 136 336"
          stroke={`url(#${gradId})`}
          strokeWidth="12"
          strokeLinecap="round"
          opacity="0.6"
        />

        <path
          d="M 164 128 L 204 128 L 204 308 L 352 308 L 324 352 L 164 352 Z"
          fill={`url(#${gradId})`}
        />

        <path
          d="M 356 156 L 268 244 L 296 272 L 384 184 Z"
          fill={`url(#${softId})`}
        />

        <line
          x1="256"
          y1="88"
          x2="256"
          y2="148"
          stroke="var(--accent)"
          strokeWidth="12"
          strokeLinecap="round"
        />
        <line
          x1="256"
          y1="364"
          x2="256"
          y2="424"
          stroke="var(--highlight)"
          strokeWidth="12"
          strokeLinecap="round"
        />
        <line
          x1="88"
          y1="256"
          x2="148"
          y2="256"
          stroke="var(--accent)"
          strokeWidth="12"
          strokeLinecap="round"
        />
        <line
          x1="364"
          y1="256"
          x2="424"
          y2="256"
          stroke="var(--highlight)"
          strokeWidth="12"
          strokeLinecap="round"
        />

        <polygon points="256,226 286,256 256,286 226,256" fill="var(--text-color)" />
        <circle cx="256" cy="256" r="10" fill="var(--bg-color)" />
      </svg>

      {showWordmark && (
        <div className="leading-tight">
          <div className="flex items-center gap-1 text-sm font-black tracking-wide">
            <span>Luair</span>
            <span className="h-1.5 w-1.5 rounded-full bg-[var(--accent)] shadow-[0_0_8px_var(--accent)]" />
          </div>
          {subtitle && (
            <div className="text-[10px] font-medium tracking-wider uppercase text-[var(--muted-text)]">
              {subtitle}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
