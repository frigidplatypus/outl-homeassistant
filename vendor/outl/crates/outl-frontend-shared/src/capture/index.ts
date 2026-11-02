/**
 * Draining the capture inbox (RFC 0363): lines captured outside the app
 * become blocks in the journal for the day they were made.
 *
 * Shared because the trigger is the same everywhere and only the writer
 * differs. On a phone the captures come from the iOS widget, the "Add to
 * today" App Intent, the iOS share extension or the Android share
 * target; on a desktop from `outl capture`, and through it from a macOS
 * Service or Shortcut, Raycast or a hotkey daemon. Either way the client
 * drains on mount, on every return to the foreground, and when the
 * native side calls `window.__outlCapturesArrived()` because something
 * wrote a capture while the app was already open.
 *
 * Must be called from inside a component body — `onCleanup` needs an
 * owner.
 */

import { onCleanup } from "solid-js";

import { drainCaptureInbox, drainMessage } from "../api/capture";

type WindowWithInbox = Window & { __outlCapturesArrived?: () => void };

export interface InboxDeps {
  /** Pull peers' ops and reload the view (`pullAndReload`). */
  pull: () => unknown;
  /** Say what landed. */
  notify: (text: string) => void;
}

/** One drain; reloads the view and says so when anything landed. */
export async function drainOnce(deps: InboxDeps): Promise<number> {
  try {
    const d = await drainCaptureInbox();
    const msg = drainMessage(d);
    if (msg) {
      await deps.pull();
      deps.notify(msg);
    }
    return d.landed;
  } catch {
    // best-effort: a capture that failed to land stays in the inbox
    return 0;
  }
}

export function installForegroundRefresh(deps: InboxDeps) {
  const onVisible = () => {
    if (document.visibilityState !== "visible") return;
    void deps.pull();
    void drainOnce(deps);
  };
  document.addEventListener("visibilitychange", onVisible);
  const win = window as WindowWithInbox;
  const arrived = () => void drainOnce(deps);
  // A desktop window regains focus without a visibility change (alt-tab
  // to a terminal and back), so focus is a trigger of its own.
  window.addEventListener("focus", arrived);
  win.__outlCapturesArrived = arrived;
  onCleanup(() => {
    document.removeEventListener("visibilitychange", onVisible);
    window.removeEventListener("focus", arrived);
    if (win.__outlCapturesArrived === arrived) delete win.__outlCapturesArrived;
  });
  void drainOnce(deps);
}
