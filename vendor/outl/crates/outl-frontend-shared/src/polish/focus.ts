/**
 * Focus mode: the chrome fades out and the outline gets a reading
 * measure. It is a view preference, so it lives on the device
 * (`UX.md` principle 5) and never touches the workspace.
 */
import { createSignal, type Accessor } from "solid-js";

export const FOCUS_KEY = "outl.focus-mode";
const ATTR = "data-outl-focus";

type Store = Pick<Storage, "getItem" | "setItem">;

function safeStore(): Store | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export interface FocusMode {
  on: Accessor<boolean>;
  set: (next: boolean) => void;
  toggle: () => void;
}

/** Read the saved preference, write it back on change, mirror it on `<html>`. */
export function createFocusMode(store: Store | null = safeStore()): FocusMode {
  const initial = store?.getItem(FOCUS_KEY) === "on";
  const [on, setOn] = createSignal(initial);
  const apply = (value: boolean) => {
    if (typeof document !== "undefined") {
      document.documentElement.setAttribute(ATTR, value ? "on" : "off");
    }
  };
  apply(initial);
  const set = (next: boolean) => {
    setOn(next);
    apply(next);
    try {
      store?.setItem(FOCUS_KEY, next ? "on" : "off");
    } catch {
      // A full or locked storage only costs persistence, not the toggle.
    }
  };
  return { on, set, toggle: () => set(!on()) };
}

let shared: FocusMode | undefined;

/** The one focus mode a client window has; both the button and any
 *  future shortcut read and flip this same signal. */
export function focusMode(): FocusMode {
  shared ??= createFocusMode();
  return shared;
}
