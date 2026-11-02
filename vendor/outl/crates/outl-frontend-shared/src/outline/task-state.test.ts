import { describe, expect, it } from "vitest";

import { taskState } from "./index";

describe("taskState", () => {
  it.each([
    ["TODO call mom", "todo"],
    ["DOING write rfc", "doing"],
    ["DONE ship", "done"],
    ["[ ] checkbox", "todo"],
    ["[/] half", "doing"],
    ["[x] ticked", "done"],
    ["[X] ticked", "done"],
    ["> TODO quoted", "todo"],
  ] as const)("reads %j as %s", (raw, state) => {
    expect(taskState(raw)).toBe(state);
  });

  it.each(["plain text", "TODOs are fun", "todo lowercase", "", "> just a quote"])(
    "reads %j as no task",
    (raw) => {
      expect(taskState(raw)).toBeNull();
    },
  );
});
