/**
 * Handlers for the speed actions (quick capture, export, focus mode), split out of
 * `action-handlers.ts` because that file sits at its size ceiling. Same
 * map, same dispatcher: `buildHandlers` spreads these in.
 */
import { exportMarkdown } from "@outl/shared/api/trust";
import { focusMode } from "@outl/shared/polish";

import { openQuickCapture } from "./quick-capture";
import type { ActionHandlers } from "./shortcuts";

export function speedHandlers(deps: { setError: (msg: string) => void }): ActionHandlers {
  return {
    // Same switch as the top bar button, so the two never disagree.
    ToggleFocusMode: () => {
      focusMode().toggle();
    },
    QuickCapture: () => {
      openQuickCapture();
    },
    // Same command the Settings → Trust card runs; the toast is the
    // only surface the palette has, so it names where the copy went.
    ExportMarkdown: async () => {
      try {
        const r = await exportMarkdown();
        deps.setError(`Exported ${r.pages} ${r.pages === 1 ? "page" : "pages"} as Markdown to ${r.dir}`);
      } catch (e) {
        deps.setError(`Export failed: ${e instanceof Error ? e.message : String(e)}`);
      }
    },
  };
}
