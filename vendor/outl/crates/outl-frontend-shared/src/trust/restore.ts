/**
 * What "restore" means for one history event.
 *
 * Every restore is **new ops**, never a rewrite of the log (root
 * `CLAUDE.md` invariant 1): putting old text back is an ordinary edit,
 * and bringing a deleted block back is an ordinary create with the text
 * it had. Both are undoable with Cmd+Z / the undo stack like any other
 * edit, and both sync like any other edit.
 *
 * Deliberately not offered:
 * - moves and property changes, where "the version before" is a
 *   position or a value whose meaning depends on its neighbours;
 * - an edit on a block that has since been deleted, where an edit would
 *   land on a trashed node the user cannot see. Its deletion row offers
 *   the text back instead.
 */

import type { TimelineEvent } from "../api/types";

export type RestorePlan =
  | { kind: "edit"; block: string; text: string }
  | { kind: "recreate"; text: string };

export function restorePlan(event: TimelineEvent, currentText?: string | null): RestorePlan | null {
  if (event.change === "edited") {
    if (event.block_deleted || event.from === null) return null;
    if (currentText !== undefined && currentText === event.from) return null;
    return { kind: "edit", block: event.block, text: event.from };
  }
  if (event.change === "deleted" && event.block_deleted && event.text) {
    return { kind: "recreate", text: event.text };
  }
  return null;
}
