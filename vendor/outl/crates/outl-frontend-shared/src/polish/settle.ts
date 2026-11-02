import { createEffect, createSignal, on, type Accessor } from "solid-js";

/**
 * True for one short beat after `busy` goes from true to false, so the
 * sync dot can play its settle pulse exactly once per finished pass.
 */
export function createSettle(busy: Accessor<boolean>, ms = 450): Accessor<boolean> {
  const [settling, setSettling] = createSignal(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  createEffect(
    on(busy, (now, prev) => {
      if (prev === true && now === false) {
        clearTimeout(timer);
        setSettling(true);
        timer = setTimeout(() => setSettling(false), ms);
      }
    }),
  );
  return settling;
}
