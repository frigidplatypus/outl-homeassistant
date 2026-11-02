import { describe, expect, it } from "vitest";
import { parkCaret, spliceText } from "./textarea";

function newTextarea(value = "", caret = 0): HTMLTextAreaElement {
  const el = document.createElement("textarea");
  document.body.appendChild(el);
  el.value = value;
  el.setSelectionRange(caret, caret);
  el.focus();
  return el;
}

describe("spliceText", () => {
  it("inserts at start without disturbing surrounding text", () => {
    const el = newTextarea("world", 0);
    spliceText(el, 0, 0, "hello ");
    expect(el.value).toBe("hello world");
  });

  it("inserts mid-text at [start, end) range", () => {
    const el = newTextarea("foobar", 3);
    spliceText(el, 3, 3, "XYZ");
    expect(el.value).toBe("fooXYZbar");
  });

  it("replaces a selected range", () => {
    const el = newTextarea("foobar", 0);
    spliceText(el, 0, 3, "BAZ");
    expect(el.value).toBe("BAZbar");
  });

  it("falls back to el.value = … when setRangeText is missing", () => {
    const el = newTextarea("ab", 0);
    // Simulate an ancient browser by deleting setRangeText.
    // @ts-expect-error testing the fallback path
    el.setRangeText = undefined;
    spliceText(el, 1, 1, "Z");
    expect(el.value).toBe("aZb");
  });
});

describe("parkCaret", () => {
  it("places the caret at the requested position", () => {
    const el = newTextarea("hello world", 0);
    parkCaret(el, 5);
    expect(el.selectionStart).toBe(5);
    expect(el.selectionEnd).toBe(5);
  });

  it("re-asserts the caret in a microtask (wins re-bindings)", async () => {
    const el = newTextarea("hello world", 0);
    parkCaret(el, 7);
    // Simulate Solid's `value={…}` binding effect running between
    // the sync and microtask phases.
    el.value = "hello world";
    el.setSelectionRange(el.value.length, el.value.length);
    // Yield to microtasks so the queued setSelectionRange runs.
    await Promise.resolve();
    expect(el.selectionStart).toBe(7);
  });

  it("does not throw when the element is detached", async () => {
    const el = newTextarea("hi", 0);
    el.remove();
    expect(() => parkCaret(el, 1)).not.toThrow();
    await Promise.resolve();
  });
});
