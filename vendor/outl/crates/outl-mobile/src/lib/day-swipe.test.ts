import { describe, expect, it } from "vitest";

import { classifySwipe, EDGE_GUARD_PX, SWIPE_MAX_MS, SWIPE_MIN_PX } from "./day-swipe";

const swipe = (dx: number, dy = 0, ms = 200, startX = 200) => classifySwipe({ startX, dx, dy, ms });

describe("classifySwipe", () => {
  it("turns the page forward on a left swipe and back on a right one", () => {
    expect(swipe(-120)).toBe("next");
    expect(swipe(120)).toBe("prev");
  });

  it("ignores a short nudge", () => {
    expect(swipe(SWIPE_MIN_PX - 1)).toBeNull();
  });

  it("ignores a mostly vertical drag, which is a scroll", () => {
    expect(swipe(80, 70)).toBeNull();
  });

  it("ignores a slow drag", () => {
    expect(swipe(150, 0, SWIPE_MAX_MS + 1)).toBeNull();
  });

  it("leaves the left edge to the system back gesture", () => {
    expect(swipe(150, 0, 200, EDGE_GUARD_PX - 1)).toBeNull();
  });
});
