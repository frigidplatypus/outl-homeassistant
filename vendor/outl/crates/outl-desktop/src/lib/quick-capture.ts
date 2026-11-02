/**
 * Quick capture: one line into today's journal from anywhere.
 *
 * Three doors open the same box: the in-app `Cmd/Ctrl+Shift+Space`
 * chord (`Action::QuickCapture`), the same chord registered with the
 * OS so it works with the window in the background, and an
 * `outl://capture` deep link. The line is appended at the end of
 * today's outline through `create_block`, so it is an op like any
 * typed block. The page on screen stays put; only when it *is* today
 * does the reply replace the view.
 */
import { createSignal } from "solid-js";

import { createBlock, openTodayJournal } from "@outl/shared/api/commands";
import type { PageView } from "@outl/shared/api/types";

const [open, setOpen] = createSignal(false);

/** Whether the capture box is on screen. */
export const quickCaptureOpen = open;
export const openQuickCapture = () => setOpen(true);
export const closeQuickCapture = () => setOpen(false);

export interface CaptureDeps {
  /** Id of the page on screen, so today's reply can replace it. */
  currentPageId: () => string | null;
  applyView: (view: PageView) => void;
}

/**
 * Append `text` to today's journal. Resolves to what happened so the
 * box can say it: `"empty"` writes nothing, `"shown"` means today is on
 * screen and was refreshed, `"saved"` means it landed off screen.
 * A backend failure rejects, and the box keeps the text.
 */
export async function captureToToday(
  text: string,
  deps: CaptureDeps,
): Promise<"empty" | "shown" | "saved"> {
  const body = text.trim();
  if (!body) return "empty";
  const today = await openTodayJournal();
  // All placement ids null = last child of the page, i.e. the end of today.
  const reply = await createBlock(today.page.id, { afterId: null, parentId: null, text: body });
  if (deps.currentPageId() === reply.view.page.id) {
    deps.applyView(reply.view);
    return "shown";
  }
  return "saved";
}
