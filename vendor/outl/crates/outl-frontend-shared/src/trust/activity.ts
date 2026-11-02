/** Labels for the sync panel's per-device rows. */

import type { ActorActivity } from "../api/trust";

/** "just now", "5 min ago", "3 h ago", "2 d ago"; "never" for 0. */
export function relativeAgo(atMs: number, nowMs: number = Date.now()): string {
  if (!atMs) return "never";
  const s = Math.max(0, Math.round((nowMs - atMs) / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 36) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}

/**
 * The op log knows actors, not device names, and an actor id is not a
 * peer id, so the honest label is "this device" or a short id. Guessing
 * a name from the peer list would be a claim the data can't back.
 */
export function deviceLabel(a: ActorActivity): string {
  return a.is_local ? "This device" : `Device ${a.actor.slice(-6).toLowerCase()}`;
}
