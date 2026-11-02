import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createNewBlockOps,
  type JournalNewBlockDeps,
} from "./Journal.new-block";
import { releaseKeyboard } from "../lib/keyboard-keeper";
import type { CreateBlockReply, PageView } from "@outl/shared/api/types";

vi.mock("../lib/haptics", () => ({ haptic: vi.fn() }));

const commands = vi.hoisted(() => ({ createBlock: vi.fn() }));
vi.mock("@outl/shared/api/commands", () => commands);

/**
 * **The focus assertions here are a proxy.** happy-dom has no software
 * keyboard, so no test in this repo can prove that iOS raised one —
 * only a physical device can. What they pin is the condition iOS keys
 * off, and the one a refactor silently breaks: a real text input is
 * focused *synchronously inside the handler*, before the `create_block`
 * round-trip resolves. Move the focus back behind an `await` and
 * `arms the keyboard before the create resolves` goes red.
 */

function pageView(): PageView {
  return {
    page: { id: "page-1", slug: "2026-10-14", title: "", kind: "journal" },
    outline: [],
  } as unknown as PageView;
}

/** A promise a test resolves by hand, so it can look at the DOM while
 *  the create is still in flight. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function harness() {
  const state = {
    editingId: null as string | null,
    commits: 0,
    applied: [] as PageView[],
    started: [] as { id: string; initial: string }[],
  };
  const deps: JournalNewBlockDeps = {
    pageId: () => "page-1",
    editingId: () => state.editingId,
    commitEdit: async () => {
      state.commits += 1;
    },
    applyView: (v) => state.applied.push(v),
    startEdit: (id, initial) => state.started.push({ id, initial }),
    withError: async (fn) => {
      try {
        return await fn();
      } catch {
        return undefined;
      }
    },
  };
  return { state, ops: createNewBlockOps(deps) };
}

function activeIsTextEntry(): boolean {
  const el = document.activeElement;
  return el?.tagName === "TEXTAREA" || el?.tagName === "INPUT";
}

beforeEach(() => {
  // The keeper is one node for the app's lifetime, so a test that
  // leaves it focused or holding text would hand that to the next one.
  releaseKeyboard();
  document.body.innerHTML = "";
  commands.createBlock.mockReset();
});

describe("appendBlock", () => {
  it("arms the keyboard before the create resolves", async () => {
    const pending = deferred<CreateBlockReply>();
    commands.createBlock.mockReturnValue(pending.promise);
    const { ops } = harness();

    const run = ops.appendBlock();

    // Still inside the handler's awaits: the block does not exist yet,
    // and a focus from here on would raise no keyboard on iOS.
    expect(activeIsTextEntry()).toBe(true);
    expect(commands.createBlock).toHaveBeenCalledTimes(1);

    pending.resolve({ view: pageView(), new_id: "blk-new" });
    await run;
  });

  it("arms before awaiting the commit of an open draft", async () => {
    const pending = deferred<CreateBlockReply>();
    commands.createBlock.mockReturnValue(pending.promise);
    const { state, ops } = harness();
    state.editingId = "blk-1";

    const run = ops.appendBlock();

    expect(activeIsTextEntry()).toBe(true);
    pending.resolve({ view: pageView(), new_id: "blk-new" });
    await run;
    expect(state.commits).toBe(1);
  });

  it("carries a keystroke typed during the handoff into the new block", async () => {
    const pending = deferred<CreateBlockReply>();
    commands.createBlock.mockReturnValue(pending.promise);
    const { state, ops } = harness();

    const run = ops.appendBlock();
    // The keyboard is up and the block's own textarea does not exist
    // yet, so iOS delivers this to whatever holds focus.
    (document.activeElement as HTMLTextAreaElement).value = "hi";
    pending.resolve({ view: pageView(), new_id: "blk-new" });
    await run;

    expect(state.started).toEqual([{ id: "blk-new", initial: "hi" }]);
  });

  it("starts the new block empty when nothing was typed", async () => {
    commands.createBlock.mockResolvedValue({
      view: pageView(),
      new_id: "blk-new",
    });
    const { state, ops } = harness();

    await ops.appendBlock();

    expect(state.started).toEqual([{ id: "blk-new", initial: "" }]);
    expect(state.applied).toHaveLength(1);
  });

  it("does not steal focus from a textarea that already has it", async () => {
    commands.createBlock.mockResolvedValue({
      view: pageView(),
      new_id: "blk-new",
    });
    const open = document.createElement("textarea");
    document.body.appendChild(open);
    open.value = "being edited";
    open.focus();
    const { state, ops } = harness();
    state.editingId = "blk-1";

    const run = ops.appendBlock();
    // The keyboard is already up; blurring this would commit the draft
    // underneath the user, and the open draft's text is not the new
    // block's text.
    expect(document.activeElement).toBe(open);
    await run;

    expect(state.started).toEqual([{ id: "blk-new", initial: "" }]);
  });

  it("drops the keyboard when the create fails", async () => {
    commands.createBlock.mockRejectedValue(new Error("no page"));
    const { state, ops } = harness();

    await ops.appendBlock();

    expect(activeIsTextEntry()).toBe(false);
    expect(state.started).toEqual([]);
  });

  it("does nothing at all without a page", async () => {
    commands.createBlock.mockResolvedValue({
      view: pageView(),
      new_id: "blk-new",
    });
    const { ops } = createNewBlockOpsWithoutPage();

    await ops.appendBlock();

    expect(commands.createBlock).not.toHaveBeenCalled();
    expect(activeIsTextEntry()).toBe(false);
  });
});

describe("createBefore", () => {
  it("arms the keyboard before the create resolves", async () => {
    const pending = deferred<CreateBlockReply>();
    commands.createBlock.mockReturnValue(pending.promise);
    const { ops } = harness();

    const run = ops.createBefore("blk-1");

    expect(activeIsTextEntry()).toBe(true);
    pending.resolve({ view: pageView(), new_id: "blk-new" });
    await run;
  });

  it("inserts with beforeId and focuses the new block", async () => {
    commands.createBlock.mockResolvedValue({
      view: pageView(),
      new_id: "blk-new",
    });
    const { state, ops } = harness();

    await ops.createBefore("blk-1");

    expect(commands.createBlock).toHaveBeenCalledWith("page-1", {
      beforeId: "blk-1",
      text: "",
    });
    expect(state.started).toEqual([{ id: "blk-new", initial: "" }]);
  });

  it("carries a keystroke typed during the handoff", async () => {
    const pending = deferred<CreateBlockReply>();
    commands.createBlock.mockReturnValue(pending.promise);
    const { state, ops } = harness();

    const run = ops.createBefore("blk-1");
    (document.activeElement as HTMLTextAreaElement).value = "x";
    pending.resolve({ view: pageView(), new_id: "blk-new" });
    await run;

    expect(state.started).toEqual([{ id: "blk-new", initial: "x" }]);
  });
});

function createNewBlockOpsWithoutPage() {
  const ops = createNewBlockOps({
    pageId: () => null,
    editingId: () => null,
    commitEdit: async () => {},
    applyView: () => {},
    startEdit: () => {},
    withError: async (fn) => fn(),
  });
  return { ops };
}
