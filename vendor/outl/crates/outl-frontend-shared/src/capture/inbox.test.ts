import { beforeEach, describe, expect, it, vi } from "vitest";
import { createRoot } from "solid-js";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { drainMessage } from "../api/capture";
import { drainOnce, installForegroundRefresh } from "./index";

const drained = (landed: number) => ({ landed, skipped: 0, page_ids: landed ? ["p"] : [] });
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("capture inbox drain", () => {
  beforeEach(() => invoke.mockReset());

  it("reloads and says so only when something landed", async () => {
    const pull = vi.fn();
    const notify = vi.fn();
    invoke.mockResolvedValueOnce(drained(0));
    expect(await drainOnce({ pull, notify })).toBe(0);
    expect(pull).not.toHaveBeenCalled();

    invoke.mockResolvedValueOnce(drained(2));
    expect(await drainOnce({ pull, notify })).toBe(2);
    expect(pull).toHaveBeenCalledOnce();
    expect(notify).toHaveBeenCalledWith("Added 2 captures to your journal.");
  });

  it("a failed drain is quiet and leaves the inbox for next time", async () => {
    const notify = vi.fn();
    invoke.mockRejectedValueOnce("loading");
    expect(await drainOnce({ pull: vi.fn(), notify })).toBe(0);
    expect(notify).not.toHaveBeenCalled();
  });

  it("drains on mount and when the native side says a capture arrived", async () => {
    invoke.mockResolvedValue(drained(0));
    const dispose = createRoot((d) => {
      installForegroundRefresh({ pull: vi.fn(), notify: vi.fn() });
      return d;
    });
    await flush();
    expect(invoke).toHaveBeenCalledWith("drain_capture_inbox");
    const before = invoke.mock.calls.length;
    (window as Window & { __outlCapturesArrived?: () => void }).__outlCapturesArrived?.();
    await flush();
    expect(invoke.mock.calls.length).toBe(before + 1);
    dispose();
    expect((window as Window & { __outlCapturesArrived?: () => void }).__outlCapturesArrived).toBeUndefined();
  });

  it("words one and many", () => {
    expect(drainMessage(drained(0))).toBeNull();
    expect(drainMessage(drained(1))).toBe("Added 1 capture to your journal.");
  });
});
