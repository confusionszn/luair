import { useEffect, useState } from "react";
import { CrosshairCanvas } from "./components/CrosshairCanvas";
import { getState, getWindowMode, isTauriRuntime, onStateUpdated } from "./lib/tauri";
import { DEFAULT_LUAIR_STATE, type LuairState } from "./types/crosshair";
import { MenuWindow } from "./windows/MenuWindow";
import { OverlayWindow } from "./windows/OverlayWindow";

function BrowserPreviewShell() {
  const [state, setState] = useState<LuairState>(DEFAULT_LUAIR_STATE);

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
    <div className="relative h-screen w-screen overflow-hidden bg-zinc-950">
      <CrosshairCanvas config={state.crosshair} />
      <div className="relative z-20 flex h-full w-full items-center justify-center p-4">
        <div className="h-[min(680px,92vh)] w-[min(980px,94vw)] overflow-hidden rounded-2xl shadow-2xl">
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
