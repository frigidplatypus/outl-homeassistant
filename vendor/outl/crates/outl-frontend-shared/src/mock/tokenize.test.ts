import { describe, expect, it } from "vitest";
import { tokenize } from "./tokenize";

describe("tokenize", () => {
  it("splits refs, tags, bold and code out of plain text", () => {
    expect(tokenize("see [[outl]] and #ui with **care** in `vite`")).toEqual([
      { kind: "plain", value: "see " },
      { kind: "ref", value: "outl" },
      { kind: "plain", value: " and " },
      { kind: "tag", value: "ui" },
      { kind: "plain", value: " with " },
      { kind: "bold", inner: [{ kind: "plain", value: "care" }] },
      { kind: "plain", value: " in " },
      { kind: "code", value: "vite" },
    ]);
  });

  it("keeps a block with no markup as one plain token", () => {
    expect(tokenize("just text")).toEqual([{ kind: "plain", value: "just text" }]);
  });
});
