import { useCallback, useRef, useState } from "react";
import { savePrefs, type Prefs } from "./prefs";

/** A persisted UI preference bundled as one unit: React state (for rendering), a
 *  synchronously-updated shadow ref (for the `[]`-deps render/keydown closures
 *  that can't read state), and localStorage persistence — all written together
 *  by the returned setter.
 *
 *  This is the ONLY correct way to declare a persisted, ref-mirrored value. A
 *  plain `setState` + `savePrefs` (the shape a naive generated toggle would take)
 *  leaves the shadow ref stale, so e.g. Fast lights up while every render stays
 *  exact — invisible in the UI and in the masked screenshots (Track E §7). The
 *  registry's `persist` fields resolve through this hook so that class of bug
 *  cannot be written.
 *
 *  Returns `[value, ref, set]`; read `value` in JSX, `ref.current` in imperative
 *  closures, and call `set` to change all three atomically. */
export function usePref<K extends keyof Prefs>(
  key: K,
  initial: Prefs[K],
): readonly [
  Prefs[K],
  React.MutableRefObject<Prefs[K]>,
  (v: Prefs[K]) => void,
] {
  const [value, setValue] = useState<Prefs[K]>(initial);
  const ref = useRef<Prefs[K]>(initial);
  const set = useCallback(
    (v: Prefs[K]) => {
      ref.current = v; // sync the shadow ref FIRST — render closures read it
      setValue(v);
      savePrefs({ [key]: v } as Partial<Prefs>);
    },
    [key],
  );
  return [value, ref, set] as const;
}
