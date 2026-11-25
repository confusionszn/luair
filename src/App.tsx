import { useEffect, useState } from "react";
import { CrosshairCanvas } from "./components/CrosshairCanvas";
import { getState, getWindowMode, isTauriRuntime, onStateUpdated } from "./lib/tauri";
import { DEFAULT_LUAIR_STATE, type LuairState } from "./types/crosshair";
import { MenuWindow } from "./windows/MenuWindow";
import { OverlayWindow } from "./windows/OverlayWindow";

function BrowserPreviewShell() {
  const [state, setState] = useState<LuairState>(DEFAULT_LUAIR_STATE);
  const isDemo =
    typeof window !== "undefined" &&
    new URLSearchParams(window.location.search).get("preview") === "demo";

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void getState().then((s) => {
      if (active) setState(s);
    });
    void onStateUpdated((next) => {
      if (active) setState(next);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-[#050811]">
      {!isDemo && <CrosshairCanvas config={state.crosshair} />}
      <div className="relative z-20 flex h-full w-full items-center justify-center p-4">
        <div className="h-[min(740px,94vh)] w-[min(1160px,95vw)] overflow-hidden rounded-2xl shadow-2xl">
          <MenuWindow />
        </div>
      </div>
    </div>
  );
}

export function App() {
  if (getWindowMode() === "overlay") {
    return <OverlayWindow />;
  }

  if (!isTauriRuntime()) {
    return <BrowserPreviewShell />;
  }

  return <MenuWindow />;
}

export default App;
