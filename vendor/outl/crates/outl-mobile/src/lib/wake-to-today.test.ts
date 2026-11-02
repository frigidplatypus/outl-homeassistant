import { describe, expect, it } from "vitest";

import { shouldWakeToToday, WAKE_AFTER_MS } from "./wake-to-today";

const base = {
  lastSeenToday: "2026-10-12",
  today: "2026-10-12",
  currentSlug: "inbox",
  hiddenForMs: 60_000,
  editing: false,
};

describe("shouldWakeToToday", () => {
  it("stays put after a short trip away", () => {
    expect(shouldWakeToToday(base)).toBe(false);
  });

  it("returns to today after a long one", () => {
    expect(shouldWakeToToday({ ...base, hiddenForMs: WAKE_AFTER_MS })).toBe(true);
  });

  it("returns to today when the date rolled over, however short the trip", () => {
    expect(
      shouldWakeToToday({ ...base, today: "2026-10-13", currentSlug: "2026-10-12" }),
    ).toBe(true);
  });

  it("never moves an open edit", () => {
    expect(
      shouldWakeToToday({ ...base, hiddenForMs: WAKE_AFTER_MS * 10, editing: true }),
    ).toBe(false);
  });

  it("does nothing when today is already on screen", () => {
    expect(
      shouldWakeToToday({ ...base, currentSlug: "2026-10-12", hiddenForMs: WAKE_AFTER_MS }),
    ).toBe(false);
  });
});
