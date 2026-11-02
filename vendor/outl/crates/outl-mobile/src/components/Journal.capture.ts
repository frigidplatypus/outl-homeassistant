/**
 * Quick capture: one line into today's journal from anywhere.
 *
 * On today itself the bottom bar's `+` just adds a block in place, the
 * way the old floating button did. Anywhere else it opens
 * `<CaptureSheet />`, and this writes the line at the end of today's
 * outline without navigating: the page on screen stays put, and only
 * when that page *is* today does the reply replace the view.
 */

import type { PageView } from "@outl/shared/api/types";
import { createBlock, openTodayJournal } from "@outl/shared/api/commands";

import { haptic } from "../lib/haptics";

export interface CaptureDeps {
  view: () => PageView | null;
  today: () => string | null;
  applyView: (v: PageView) => void;
  withError: <T>(fn: () => Promise<T>) => Promise<T | undefined>;
  /** A short confirmation, through the same toast as every other message. */
  notify: (text: string) => void;
}

export function createCapture(deps: CaptureDeps) {
  /** Whether the `+` should add in place rather than open the sheet. */
  const onToday = () => {
    const v = deps.view();
    return v !== null && v.page.kind === "journal" && v.page.slug === deps.today();
  };

  /** Append `text` to today's journal. `true` once it is in the log. */
  async function captureToToday(text: string): Promise<boolean> {
    const body = text.trim();
    if (!body) return false;
    const today = await deps.withError(openTodayJournal);
    if (!today) return false;
    const reply = await deps.withError(() =>
      createBlock(today.page.id, { afterId: null, parentId: null, text: body }),
    );
    if (!reply) return false;
    haptic("success");
    if (deps.view()?.page.id === reply.view.page.id) deps.applyView(reply.view);
    else deps.notify("Added to today's journal.");
    return true;
  }

  return { onToday, captureToToday };
}
