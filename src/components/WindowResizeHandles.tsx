import { startResizeDragging, type ResizeDirection } from "../lib/tauri";

interface WindowResizeHandlesProps {
  disabled?: boolean;
}

const HANDLES: {
  direction: ResizeDirection;
  className: string;
}[] = [
  { direction: "North", className: "top-0 left-3 right-3 h-1.5 cursor-ns-resize" },
  { direction: "South", className: "bottom-0 left-3 right-3 h-1.5 cursor-ns-resize" },
  { direction: "West", className: "left-0 top-3 bottom-3 w-1.5 cursor-ew-resize" },
  { direction: "East", className: "right-0 top-3 bottom-3 w-1.5 cursor-ew-resize" },
  { direction: "NorthWest", className: "top-0 left-0 h-3 w-3 cursor-nwse-resize" },
  { direction: "NorthEast", className: "top-0 right-0 h-3 w-3 cursor-nesw-resize" },
  { direction: "SouthWest", className: "bottom-0 left-0 h-3 w-3 cursor-nesw-resize" },
  { direction: "SouthEast", className: "bottom-0 right-0 h-3.5 w-3.5 cursor-nwse-resize" },
];

export function WindowResizeHandles({ disabled = false }: WindowResizeHandlesProps) {
  if (disabled) return null;

  return (
    <>
      {HANDLES.map((h) => (
        <div
          key={h.direction}
          onMouseDown={(e) => {
            if (e.button === 0) {
              e.preventDefault();
              void startResizeDragging(h.direction);
            }
          }}
          className={`absolute z-50 select-none ${h.className}`}
          aria-hidden="true"
        />
      ))}

      {/* Subtle corner grip visual cue */}
      <svg
        width="12"
        height="12"
        viewBox="0 0 12 12"
        className="pointer-events-none absolute right-1.5 bottom-1.5 z-40 opacity-35"
        aria-hidden="true"
      >
        <path
          d="M11 3L3 11M11 7L7 11"
          stroke="var(--text-color)"
          strokeWidth="1.25"
          strokeLinecap="round"
        />
      </svg>
    </>
  );
}
