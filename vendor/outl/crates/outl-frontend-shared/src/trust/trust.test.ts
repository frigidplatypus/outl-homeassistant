import { describe, expect, it, vi } from "vitest";
import { createRoot } from "solid-js";

import type { TimelineEvent } from "../api/types";
import { deviceLabel, relativeAgo } from "./activity";
import { wordDiff } from "./diff";
import { restorePlan } from "./restore";
import { createSavedMarks } from "./saved";
import { visibleMerges, withoutMerge } from "./merges";

const ev = (over: Partial<TimelineEvent>): TimelineEvent => ({
  at_ms: 1,
  actor: "A",
  block: "B1",
  block_deleted: false,
  change: "edited",
  from: "old text",
  to: "new text",
  text: null,
  key: null,
  ...over,
});

describe("wordDiff", () => {
  it("marks the replaced word and keeps the rest", () => {
    expect(wordDiff("buy milk today", "buy oat milk today")).toEqual([
      { op: "same", text: "buy " },
      { op: "added", text: "oat " },
      { op: "same", text: "milk today" },
    ]);
  });

  it("round-trips: same+removed is from, same+added is to", () => {
    const from = "the  quick brown fox\njumps";
    const to = "a quick red fox\n\njumps high";
    const segs = wordDiff(from, to);
    expect(segs.filter((s) => s.op !== "added").map((s) => s.text).join("")).toBe(from);
    expect(segs.filter((s) => s.op !== "removed").map((s) => s.text).join("")).toBe(to);
  });

  it("handles empty sides", () => {
    expect(wordDiff("", "hi")).toEqual([{ op: "added", text: "hi" }]);
    expect(wordDiff("hi", "")).toEqual([{ op: "removed", text: "hi" }]);
    expect(wordDiff("", "")).toEqual([]);
  });

  it("falls back to a coarse diff on huge blocks", () => {
    const big = Array.from({ length: 700 }, (_, i) => `w${i}`).join(" ");
    const segs = wordDiff(big, big + " x");
    expect(segs.map((s) => s.op)).toEqual(["removed", "added"]);
  });
});

describe("restorePlan", () => {
  it("an edit puts the previous text back", () => {
    expect(restorePlan(ev({}))).toEqual({ kind: "edit", block: "B1", text: "old text" });
  });

  it("an empty previous text is still a version", () => {
    expect(restorePlan(ev({ from: "" }))).toEqual({ kind: "edit", block: "B1", text: "" });
  });

  it("the first write has nothing before it", () => {
    expect(restorePlan(ev({ from: null }))).toBeNull();
  });

  it("never edits a block that is in the trash", () => {
    expect(restorePlan(ev({ block_deleted: true }))).toBeNull();
  });

  it("skips a no-op restore when the block already says that", () => {
    expect(restorePlan(ev({}), "old text")).toBeNull();
  });

  it("a deletion brings the text back as a new block", () => {
    expect(
      restorePlan(ev({ change: "deleted", block_deleted: true, from: null, to: null, text: "lost" })),
    ).toEqual({ kind: "recreate", text: "lost" });
  });

  it("a deletion that was already undone offers nothing", () => {
    expect(restorePlan(ev({ change: "deleted", block_deleted: false, text: "x" }))).toBeNull();
  });

  it("moves and properties are not restorable", () => {
    expect(restorePlan(ev({ change: "moved" }))).toBeNull();
    expect(restorePlan(ev({ change: "property", key: "k" }))).toBeNull();
  });
});

describe("activity labels", () => {
  it("relativeAgo buckets", () => {
    const now = 10_000_000;
    expect(relativeAgo(0, now)).toBe("never");
    expect(relativeAgo(now - 10_000, now)).toBe("just now");
    expect(relativeAgo(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(relativeAgo(now - 3 * 3_600_000, now)).toBe("3 h ago");
    expect(relativeAgo(now - 3 * 86_400_000, now - 0)).toBe("3 d ago");
    expect(relativeAgo(now + 5000, now)).toBe("just now");
  });

  it("names only what the data backs", () => {
    const a = { actor: "01JQ00000000000000000ABCDEF", ops: 1, last_at_ms: 1, is_local: false };
    expect(deviceLabel(a)).toBe("Device abcdef");
    expect(deviceLabel({ ...a, is_local: true })).toBe("This device");
  });
});

describe("createSavedMarks", () => {
  it("marks, then fades on its own, and a re-mark restarts the window", () => {
    vi.useFakeTimers();
    createRoot((dispose) => {
      const marks = createSavedMarks(1000);
      marks.mark("b");
      expect(marks.isSaved("b")).toBe(true);
      expect(marks.isSaved("c")).toBe(false);
      vi.advanceTimersByTime(800);
      marks.mark("b");
      vi.advanceTimersByTime(800);
      expect(marks.isSaved("b")).toBe(true);
      vi.advanceTimersByTime(300);
      expect(marks.isSaved("b")).toBe(false);
      dispose();
    });
    vi.useRealTimers();
  });
});

describe("merged edits log", () => {
  const m = (id: string, ours: string, merged: string) => ({
    id,
    page_id: "p",
    page_title: "Inbox",
    block_id: "b",
    ours,
    merged,
    peer_actor: "01JQ00000000000000009F3A2C",
    at_ms: 1,
  });

  it("hides merges that left the text unchanged", () => {
    expect(visibleMerges([m("a", "x", "x y"), m("b", "same", "same")]).map((e) => e.id)).toEqual(["a"]);
  });

  it("drops only the undone entry and keeps the order", () => {
    const list = [m("a", "1", "2"), m("b", "3", "4"), m("c", "5", "6")];
    expect(withoutMerge(list, "b").map((e) => e.id)).toEqual(["a", "c"]);
  });
});
