import { useEffect, useState } from "react";
import { CrosshairCanvas } from "../components/CrosshairCanvas";
import { getState, onFireState, onStateUpdated } from "../lib/tauri";
import { DEFAULT_LUAIR_STATE, type LuairState } from "../types/crosshair";

export function OverlayWindow() {
  const [state, setState] = useState<LuairState>(DEFAULT_LUAIR_STATE);
  const [isFiring, setIsFiring] = useState(false);

  useEffect(() => {
    let active = true;
    let unlistenState: (() => void) | undefined;
    let unlistenFire: (() => void) | undefined;

    void getState()
      .then((initial) => {
        if (active) setState(initial);
      })
      .catch(() => {});

    void onStateUpdated((next) => {
      if (active) setState(next);
    }).then((fn) => {
      unlistenState = fn;
    });

    void onFireState((firing) => {
      if (active) setIsFiring(firing);
    }).then((fn) => {
      unlistenFire = fn;
    });

    return () => {
      active = false;
      unlistenState?.();
      unlistenFire?.();
    };
  }, []);

  return (
    <div className="pointer-events-none h-screen w-screen overflow-hidden bg-transparent">
      <CrosshairCanvas config={state.crosshair} firing={isFiring} />
    </div>
  );
}
