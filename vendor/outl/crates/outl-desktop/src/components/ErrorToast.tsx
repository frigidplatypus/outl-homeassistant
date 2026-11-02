import { Show } from "solid-js";

import { appState, setAppState } from "../lib/store";

/**
 * Floating error toast, top-right (the OS "notification" corner).
 *
 * Replaces the full-width banner that used to sit at the bottom of
 * `<OutlineView />`: that banner rendered in the document flow at the
 * base of `<main>`, so the fixed `<ChromeToggleBar />` (bottom-left,
 * `z-20`) painted over its left edge — the error text was half-hidden
 * behind the toggle cluster.
 *
 * Anchoring the toast `fixed` top-right with `z-50` (the same tier as
 * the picker / settings overlays) and mounting it **last** in the shell
 * keeps it above every chrome element, so nothing ever covers it. The
 * card itself is the only pointer target (`pointer-events-none` wrapper,
 * `pointer-events-auto` card) so it never blocks clicks on the outline
 * underneath when it's wide.
 */
export function ErrorToast() {
  return (
    <Show when={appState.lastError}>
      <div class="pointer-events-none fixed right-4 top-16 z-50 flex max-w-sm justify-end">
        <div class="pointer-events-auto flex items-start gap-2 rounded-lg border border-(--color-outl-status-message-fg)/30 bg-(--color-outl-bg-elev) px-3 py-2 text-xs text-(--color-outl-status-message-fg) shadow-lg">
          <span aria-hidden="true">⚠</span>
          <span class="min-w-0 break-words">{appState.lastError}</span>
          <button
            type="button"
            onClick={() => setAppState("lastError", null)}
            class="ml-1 shrink-0 opacity-70 hover:opacity-100"
            aria-label="Dismiss error"
          >
            ✕
          </button>
        </div>
      </div>
    </Show>
  );
}
