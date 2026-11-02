import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PageView } from "@outl/shared/api/types";

const commands = vi.hoisted(() => ({ openTodayJournal: vi.fn(), createBlock: vi.fn() }));
vi.mock("@outl/shared/api/commands", () => commands);

import {
  captureToToday,
  closeQuickCapture,
  openQuickCapture,
  quickCaptureOpen,
} from "./quick-capture";

const view = (id: string) => ({ page: { id }, outline: [] }) as unknown as PageView;

beforeEach(() => {
  commands.openTodayJournal.mockReset().mockResolvedValue(view("today"));
  commands.createBlock.mockReset().mockResolvedValue({ view: view("today"), new_id: "b1" });
});

describe("captureToToday", () => {
  it("writes nothing for blank text", async () => {
    const applyView = vi.fn();
    expect(await captureToToday("   \n", { currentPageId: () => "x", applyView })).toBe("empty");
    expect(commands.openTodayJournal).not.toHaveBeenCalled();
    expect(commands.createBlock).not.toHaveBeenCalled();
  });

  it("appends the trimmed line at the end of today", async () => {
    await captureToToday("  buy milk  ", { currentPageId: () => "other", applyView: vi.fn() });
    expect(commands.createBlock).toHaveBeenCalledWith("today", {
      afterId: null,
      parentId: null,
      text: "buy milk",
    });
  });

  it("leaves another page on screen alone", async () => {
    const applyView = vi.fn();
    expect(await captureToToday("x", { currentPageId: () => "other", applyView })).toBe("saved");
    expect(applyView).not.toHaveBeenCalled();
  });

  it("refreshes today when today is on screen", async () => {
    const applyView = vi.fn();
    expect(await captureToToday("x", { currentPageId: () => "today", applyView })).toBe("shown");
    expect(applyView).toHaveBeenCalledWith(view("today"));
  });

  it("rejects on a backend failure so the box keeps the text", async () => {
    commands.createBlock.mockRejectedValue(new Error("disk full"));
    await expect(
      captureToToday("x", { currentPageId: () => null, applyView: vi.fn() }),
    ).rejects.toThrow("disk full");
  });
});

describe("open state", () => {
  it("opens and closes", () => {
    openQuickCapture();
    expect(quickCaptureOpen()).toBe(true);
    closeQuickCapture();
    expect(quickCaptureOpen()).toBe(false);
  });
});
