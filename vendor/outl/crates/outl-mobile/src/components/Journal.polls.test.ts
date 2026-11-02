import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRoot } from "solid-js";

const commands = vi.hoisted(() => ({ deliverDueReminders: vi.fn(async () => {}) }));
vi.mock("@outl/shared/api/commands", () => commands);

const reminders = vi.hoisted(() => ({ setup: vi.fn(), stop: vi.fn() }));
vi.mock("../lib/reminder-notifications", () => ({
  setupReminderNotifications: (...a: unknown[]) => reminders.setup(...a),
}));

import { installJournalPolls, type JournalPollDeps } from "./Journal.polls";

function harness(over: Partial<JournalPollDeps> = {}) {
  const calls = {
    setOnline: vi.fn(),
    refreshPeerStatus: vi.fn(async () => {}),
    pullAndReload: vi.fn(async () => {}),
    navigateToBlock: vi.fn(async () => {}),
    setError: vi.fn(),
  };
  const deps: JournalPollDeps = { ...calls, ...over };
  let polls!: ReturnType<typeof installJournalPolls>;
  const dispose = createRoot((d) => {
    polls = installJournalPolls(deps);
    return d;
  });
  return { polls, calls, dispose };
}

beforeEach(() => {
  vi.useFakeTimers();
  commands.deliverDueReminders.mockReset();
  commands.deliverDueReminders.mockResolvedValue(undefined);
  reminders.setup.mockReset();
  reminders.setup.mockResolvedValue(reminders.stop);
  reminders.stop.mockReset();
});

afterEach(() => vi.useRealTimers());

describe("journal polls", () => {
  /**
   * The whole reason this module exists. `peer_status` is a full workspace
   * load and, on iOS, runs on the main thread the UI paints from — so the
   * poll that used to start from the component *body* queued it ahead of
   * the journal's own first paint. 9 of 9 crash reports had the main thread
   * inside it (FRONTBOARD 0x8BADF00D).
   */
  it("installing touches no backend; only startPolling does", () => {
    const { calls } = harness();

    expect(calls.refreshPeerStatus).not.toHaveBeenCalled();
    expect(calls.pullAndReload).not.toHaveBeenCalled();
    expect(reminders.setup).not.toHaveBeenCalled();
    vi.advanceTimersByTime(60_000);
    expect(calls.refreshPeerStatus).not.toHaveBeenCalled();
  });

  it("polls peers and pulls once started", () => {
    const { polls, calls } = harness();
    polls.startPolling();

    expect(calls.refreshPeerStatus).toHaveBeenCalledTimes(1); // immediate read
    vi.advanceTimersByTime(3000);
    expect(calls.refreshPeerStatus).toHaveBeenCalledTimes(2);
    expect(calls.pullAndReload).toHaveBeenCalledWith({ background: true });
  });

  it("starting twice does not double the timers", () => {
    const { polls, calls } = harness();
    polls.startPolling();
    polls.startPolling();

    vi.advanceTimersByTime(3000);
    expect(calls.pullAndReload).toHaveBeenCalledTimes(1);
  });

  it("delivers reminders on the slow tick, and a refused delivery stays quiet", async () => {
    commands.deliverDueReminders.mockRejectedValue("no permission");
    const { polls, calls } = harness();
    polls.startPolling();

    await vi.advanceTimersByTimeAsync(30_000);
    expect(commands.deliverDueReminders).toHaveBeenCalledTimes(1);
    expect(calls.setError).not.toHaveBeenCalled();
  });

  /** A failed action-type registration leaves a reminder without buttons;
   *  an unhandled rejection would leave it with a console trace too. */
  it("a failed notification-action registration is handled", async () => {
    reminders.setup.mockRejectedValue(new Error("no entitlement"));
    const { polls, calls } = harness();
    polls.startPolling();

    await vi.advanceTimersByTimeAsync(0);
    expect(calls.setError).not.toHaveBeenCalled();
  });

  /** Cleanup is armed before `startPolling` can run, so an unmount after a
   *  FAILED boot — where it never ran — still tears down the listeners. */
  it("cleans up whether or not polling ever started", () => {
    const never = harness();
    expect(() => never.dispose()).not.toThrow();

    const started = harness();
    started.polls.startPolling();
    started.dispose();
    started.calls.refreshPeerStatus.mockClear();
    vi.advanceTimersByTime(30_000);
    expect(started.calls.refreshPeerStatus).not.toHaveBeenCalled();
  });
});
