/**
 * Per-block "saved to disk" marks. `Journal`'s commit marks a block once
 * `edit_block` resolves (the op is in this device's log and the `.md`
 * is rewritten); `BlockRow` renders the fading dot. The desktop keeps
 * the same singleton in its store.
 */
import { createSavedMarks } from "@outl/shared/trust";

export const savedMarks = createSavedMarks();
