import { describe, expect, it } from "vitest";

import { MOD_CTRL, MOD_META, MOD_SHIFT } from "./api";
import { eventToChord } from "./shortcuts";

const ev = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("eventToChord", () => {
  it("keeps SHIFT on Space, so Cmd/Ctrl+Shift+Space reaches QuickCapture", () => {
    expect(eventToChord(ev({ key: " ", metaKey: true, shiftKey: true }))).toEqual({
      mods: MOD_META | MOD_SHIFT,
      key: { kind: "Space" },
    });
    expect(eventToChord(ev({ key: " ", ctrlKey: true, shiftKey: true }))?.mods).toBe(
      MOD_CTRL | MOD_SHIFT,
    );
  });

  it("still drops SHIFT on a shifted symbol like ?", () => {
    expect(eventToChord(ev({ key: "?", shiftKey: true }))).toEqual({
      mods: 0,
      key: { kind: "Char", value: "?" },
    });
  });
});
