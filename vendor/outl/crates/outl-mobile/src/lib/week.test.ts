import { describe, expect, it } from "vitest";

import { weekOf } from "./week";

describe("weekOf", () => {
  it("lists Sunday through Saturday around the day", () => {
    expect(weekOf("2026-10-14")).toEqual([
      "2026-10-11",
      "2026-10-12",
      "2026-10-13",
      "2026-10-14",
      "2026-10-15",
      "2026-10-16",
      "2026-10-17",
    ]);
  });

  it("crosses a month and a year boundary", () => {
    expect(weekOf("2026-01-01")[0]).toBe("2025-12-28");
    expect(weekOf("2026-01-01")[6]).toBe("2026-01-03");
  });

  it("is empty for a page that is not a day", () => {
    expect(weekOf("inbox")).toEqual([]);
    expect(weekOf(null)).toEqual([]);
  });
});
