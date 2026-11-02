/**
 * The per-block "saved to disk" mark.
 *
 * A block is marked the moment its commit command resolves, which is
 * the moment the op is in this device's `ops-<actor>.jsonl` and the
 * page's `.md` is rewritten. The mark fades on its own after `holdMs`;
 * it is reassurance, not chrome, so it never stays on screen.
 */

import { createSignal } from "solid-js";

export interface SavedMarks {
  /** Mark `id` as just saved. */
  mark: (id: string) => void;
  /** Whether `id` is inside its fade window. Reactive. */
  isSaved: (id: string) => boolean;
}

export function createSavedMarks(holdMs = 1600): SavedMarks {
  const [saved, setSaved] = createSignal<ReadonlySet<string>>(new Set());
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  return {
    mark(id) {
      const prev = timers.get(id);
      if (prev) clearTimeout(prev);
      setSaved((s) => new Set(s).add(id));
      timers.set(
        id,
        setTimeout(() => {
          timers.delete(id);
          setSaved((s) => {
            const next = new Set(s);
            next.delete(id);
            return next;
          });
        }, holdMs),
      );
    },
    isSaved: (id) => saved().has(id),
  };
}
