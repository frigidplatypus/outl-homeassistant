/** Pure helpers for the merged-edits log. */

import type { MergedEdit } from "../api/trust";

/** Merges worth showing: the ones where the text actually changed. */
export function visibleMerges(list: MergedEdit[]): MergedEdit[] {
  return list.filter((m) => m.ours !== m.merged);
}

/** The log after an undo: that entry gone, order kept. */
export function withoutMerge(list: MergedEdit[], id: string): MergedEdit[] {
  return list.filter((m) => m.id !== id);
}
