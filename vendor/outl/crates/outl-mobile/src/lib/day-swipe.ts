/**
 * Horizontal swipe between journal days, on the header and the week
 * strip only. Rows below own their own horizontal swipe (`SwipeRow`),
 * so the outline never listens for this one: two gestures on one
 * surface is the collision `UX.md` rules out.
 */

/** Distance a finger must travel sideways before it counts. */
export const SWIPE_MIN_PX = 56;
/** A slower drag is reading, not swiping. */
export const SWIPE_MAX_MS = 700;
/** Touches starting this close to the left edge belong to the OS back gesture. */
export const EDGE_GUARD_PX = 24;

export type DaySwipe = "prev" | "next" | null;

/**
 * What a finished touch meant. Swiping left (content moves left) brings
 * the next day in, the way a page turns; swiping right goes back.
 */
export function classifySwipe(s: {
  startX: number;
  dx: number;
  dy: number;
  ms: number;
}): DaySwipe {
  if (s.startX < EDGE_GUARD_PX) return null;
  if (s.ms > SWIPE_MAX_MS) return null;
  if (Math.abs(s.dx) < SWIPE_MIN_PX) return null;
  if (Math.abs(s.dx) < Math.abs(s.dy) * 1.5) return null;
  return s.dx < 0 ? "next" : "prev";
}

/** Touch handlers to spread on the surface that should answer the swipe. */
export function createDaySwipe(on: { prev: () => void; next: () => void }) {
  let start: { x: number; y: number; t: number } | null = null;
  return {
    onTouchStart(e: TouchEvent) {
      const t = e.touches[0];
      start = t && e.touches.length === 1 ? { x: t.clientX, y: t.clientY, t: Date.now() } : null;
    },
    onTouchEnd(e: TouchEvent) {
      const t = e.changedTouches[0];
      if (!start || !t) return;
      const verdict = classifySwipe({
        startX: start.x,
        dx: t.clientX - start.x,
        dy: t.clientY - start.y,
        ms: Date.now() - start.t,
      });
      start = null;
      if (verdict === "prev") on.prev();
      else if (verdict === "next") on.next();
    },
    onTouchCancel() {
      start = null;
    },
  };
}
