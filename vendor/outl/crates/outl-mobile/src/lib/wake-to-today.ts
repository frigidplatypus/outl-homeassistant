/**
 * Opening the app lands on today (`UX.md` principle 1), and that has
 * to hold for a phone that was backgrounded, not just a cold start:
 * iOS keeps the webview alive for days, so without this the app reopens
 * on whatever page it was left on, often yesterday.
 */

/** Away this long and the next foreground counts as opening the app. */
export const WAKE_AFTER_MS = 30 * 60 * 1000;

/**
 * Whether coming back to the foreground should move the user to today.
 * Never mid-edit: the open textarea is the user's, and moving it away
 * would be the interruption principle 6 forbids.
 */
export function shouldWakeToToday(s: {
  /** Today's slug when the app went to the background. */
  lastSeenToday: string | null;
  /** Today's slug now, from the backend. */
  today: string;
  /** Slug on screen. */
  currentSlug: string | null;
  hiddenForMs: number;
  editing: boolean;
}): boolean {
  if (s.editing) return false;
  if (s.currentSlug === s.today) return false;
  if (s.lastSeenToday !== null && s.lastSeenToday !== s.today) return true;
  return s.hiddenForMs >= WAKE_AFTER_MS;
}
