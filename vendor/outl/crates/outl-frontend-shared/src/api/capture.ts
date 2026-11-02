/**
 * The capture inbox (RFC 0363): a line captured outside the app waits in
 * a folder until the app drains it into the journal.
 *
 * Six writers on three platforms feed it — iOS's widget, "Add to today"
 * App Intent and share extension; Android's share target, widget and
 * quick settings tile; and `outl capture` from a terminal, which is what
 * gives the desktop a route. Listed in the wire-mirror files
 * (`outl-tauri-shared/tests/ts_parser`).
 */

import { invoke } from "@tauri-apps/api/core";

/** What one drain did. Mirrors `CaptureDrainDto` in `outl-tauri-shared`. */
export interface CaptureDrain {
  landed: number;
  skipped: number;
  page_ids: string[];
}

/** Land every waiting capture. Zeros when there is no inbox. */
export function drainCaptureInbox(): Promise<CaptureDrain> {
  return invoke<CaptureDrain>("drain_capture_inbox");
}

/**
 * The toast a drain earns, or `null` when there is nothing to say.
 *
 * `skipped` is reported, not just `landed`. It counts files the drain
 * could not read or parse, which it leaves in the inbox — so without a
 * word here the capture simply never appears, and the only other trace
 * is a `tracing::warn!` nobody reads. A capture that vanishes quietly is
 * the failure RFC 0363 exists to prevent, and this repo's rule is that a
 * refusal has to reach the user.
 */
export function drainMessage(d: CaptureDrain): string | null {
  const landed =
    d.landed === 0
      ? null
      : d.landed === 1
        ? "Added 1 capture to your journal."
        : `Added ${d.landed} captures to your journal.`;
  if (d.skipped === 0) return landed;
  const skipped =
    d.skipped === 1
      ? "1 capture couldn't be read and is still waiting."
      : `${d.skipped} captures couldn't be read and are still waiting.`;
  return landed === null ? skipped : `${landed} ${skipped}`;
}
