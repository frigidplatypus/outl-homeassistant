/**
 * The journal's background tickers and OS connectivity signals: the
 * online/offline listeners, the iroh peer-health + pull poll, reminder
 * delivery, and the notification-action bridge.
 *
 * **Armed in the body, started after the boot load.** That split is the
 * whole reason this is a module. On iOS `wry` serves Tauri IPC from the
 * main thread and a non-`async` command runs there, so a poll started
 * from the component *body* — i.e. during render — queues a full
 * workspace load (`peer_status`) on the UI thread ahead of the journal's
 * own first paint. The app was SIGKILLed by the scene-update watchdog
 * (0x8BADF00D) with the main thread inside exactly that command.
 *
 * So `installJournalPolls` registers only the free things synchronously
 * (two DOM listeners, and `onCleanup` for everything it may later own)
 * and hands back `startPolling`, which `onMount` calls once the boot
 * load has settled. Cleanup is armed up front and checks what it owns,
 * so an unmount after a *failed* boot — where `startPolling` never ran —
 * still tears down cleanly.
 *
 * Must be called from inside the component body: `onCleanup` needs a
 * Solid owner, and there is deliberately no `runWithOwner` wrapper
 * hiding that.
 */

import { onCleanup } from "solid-js";

import { deliverDueReminders } from "@outl/shared/api/commands";

import { setupReminderNotifications } from "../lib/reminder-notifications";

/** Peer health + pull cadence. Short because this is what makes a desktop
 *  or TUI edit show up without the user touching refresh. */
const PEER_POLL_MS = 3000;
/** Reminder delivery cadence. The schedule has minute granularity, so a
 *  3s tick would buy nothing and cost a command every 3s. */
const REMINDER_POLL_MS = 30_000;

export interface JournalPollDeps {
  setOnline: (value: boolean) => void;
  /** Probe iroh peer health (best-effort; keeps the last value on failure). */
  refreshPeerStatus: () => Promise<void>;
  /** The guarded reload path — pull, then re-render behind its generation guard. */
  pullAndReload: (opts: { background: boolean }) => Promise<void>;
  /** Open a reminder's page and scroll its block into view (notification tap). */
  navigateToBlock: (slug: string, blockId: string) => Promise<void>;
  setError: (message: string) => void;
}

export interface JournalPolls {
  /** Start the timers. Idempotent, so a second boot path can't double them. */
  startPolling: () => void;
}

export function installJournalPolls(deps: JournalPollDeps): JournalPolls {
  if (typeof window === "undefined") return { startPolling: () => {} };

  // Connectivity: the SECONDARY sync signal (truly no radio → no peer can
  // be up anyway). Pure DOM side-effects, but they must be registered and
  // torn down within the component's owner.
  const upOnline = () => deps.setOnline(true);
  const upOffline = () => deps.setOnline(false);
  window.addEventListener("online", upOnline);
  window.addEventListener("offline", upOffline);

  let peerPoll: number | undefined;
  let reminderPoll: number | undefined;
  let stopReminderActions: (() => void) | undefined;

  onCleanup(() => {
    window.removeEventListener("online", upOnline);
    window.removeEventListener("offline", upOffline);
    if (peerPoll !== undefined) window.clearInterval(peerPoll);
    if (reminderPoll !== undefined) window.clearInterval(reminderPoll);
    stopReminderActions?.();
  });

  function startPolling() {
    if (peerPoll !== undefined) return;
    // Probe iroh peer health now, then on the tick, so the dot tracks the
    // mesh without a user action. `peer-ops-changed` (the native ops bridge)
    // and a force-sync also poke `refreshPeerStatus` for a fresher read.
    void deps.refreshPeerStatus();
    peerPoll = window.setInterval(() => {
      void deps.refreshPeerStatus();
      // Pull from peers AND reload the view every tick so an edit on the
      // desktop OR the TUI shows up without the refresh button. The mobile side
      // initiating the dial is NAT-friendly (waiting for the desktop to reach an
      // iPhone behind carrier NAT is not), which is why desktop/TUI→mobile needs
      // us to pull. We call the full `pullAndReload` (not just `syncNow`):
      // relying on the `workspace-ready` event alone left the ops on disk
      // without re-rendering — the symptom was "only shows after I hit sync".
      //
      // NOT guarded on `editingId()` here. `pullAndReload` already handles the
      // editing case correctly — it pulls the peer's ops to disk and defers
      // only the RE-RENDER, which the `editingId` effect drains the moment the
      // user leaves the field. Testing it out here too made that branch
      // unreachable and turned "don't reset the textarea" into "don't sync at
      // all while a block is open", which is precisely the state a user is in
      // while waiting for a desktop edit to show up.
      void deps.pullAndReload({ background: true });
    }, PEER_POLL_MS);

    // Make the banner actionable: "Snooze 1h" / "Done" buttons, and a plain
    // tap that lands on the block instead of the journal. Registered BEFORE
    // the delivery poll is armed — iOS resolves the category at delivery
    // time, and an unregistered one shows as a banner with no buttons and no
    // error.
    void setupReminderNotifications({
      navigateToBlock: deps.navigateToBlock,
      onError: deps.setError,
    })
      .then((stop) => {
        stopReminderActions = stop;
      })
      .catch(() => {
        // Registration is best-effort: without it a reminder still arrives,
        // it just carries no action buttons. A toast about it on every boot
        // would be worse than the missing buttons.
      });

    // Reminder delivery. The backend decides what is due and remembers what
    // this device already delivered, so a poll that fires twice never
    // double-buzzes and a phone that was asleep owes one banner, not a
    // backlog. It short-circuits when reminders are off, so this ticks
    // unconditionally rather than re-subscribing on a settings change.
    reminderPoll = window.setInterval(() => {
      void deliverDueReminders().catch(() => {
        // Permission not granted yet — the Rust side logged it, and a
        // toast every 30 seconds would be worse than silence.
      });
    }, REMINDER_POLL_MS);
  }

  return { startPolling };
}
