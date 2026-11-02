import { Show, createSignal, onCleanup, onMount } from "solid-js";

import {
  peerStatus,
  reloadWorkspace,
  syncNow,
} from "@outl/shared/api/commands";
import { peersOnline } from "@outl/shared/peers";
import { createSettle } from "@outl/shared/polish";

import { setAppState } from "../lib/store";
import { onPeerOpsChanged } from "../lib/events";

/**
 * Always-visible sync status dot for the desktop chrome cluster.
 *
 * Mirrors the mobile `<SyncDot>`: green when at least one iroh peer is
 * reachable, orange when none are, a dim dot while the first probe is
 * still in flight, a spinner while a sync pass runs.
 *
 * Clicking it syncs now: dial every peer, reload the op log so the page
 * re-renders with whatever arrived. That is the refresh the chrome used
 * to need a separate button for. Clicks while a pass is in flight are
 * ignored, so mashing the dot never stacks dials. Right-click opens
 * Settings → Sync, where the full peer list + pairing live
 * (`<SyncPanel>` is the Sync section of `<SettingsModal>`).
 *
 * The reachability read is the running transport's own dial outcomes
 * (`peerStatus()` → `peersOnline`) — the SAME source the Settings Sync
 * panel uses, so the dot and the panel can never disagree. We re-probe on
 * a slow interval and immediately whenever a peer delivers ops
 * (`peer-ops-changed`): a delivery is proof the mesh is up, so the dot
 * should flip green without waiting for the next tick.
 */
export function SyncIndicator() {
  // null = first probe still running (dim); true / false = reachability.
  const [online, setOnline] = createSignal<boolean | null>(null);
  const [syncing, setSyncing] = createSignal(false);
  // One soft pulse on the dot when a pass finishes, same as mobile.
  const settling = createSettle(syncing);
  // When this window last saw a sync land: a pass it ran finished, or a
  // peer delivered ops. `null` until the first one, the tooltip says so.
  const [lastSynced, setLastSynced] = createSignal<number | null>(null);
  // Ticks the "2 min ago" text forward while the tooltip may be read.
  const [now, setNow] = createSignal(Date.now());

  async function refresh() {
    try {
      setOnline(peersOnline(await peerStatus()));
    } catch {
      // Probe failed (no transport wired / no network) — read as offline
      // rather than blanking, so the dot stays meaningful.
      setOnline(false);
    }
  }

  onMount(async () => {
    void refresh();
    // Re-probe on the catch-up cadence; cheap (reads cached dial health).
    const id = setInterval(() => {
      setNow(Date.now());
      void refresh();
    }, 8000);
    onCleanup(() => clearInterval(id));
    const unlisten = await onPeerOpsChanged(() => {
      setLastSynced(Date.now());
      void refresh();
    });
    onCleanup(() => unlisten());
  });

  async function syncNowFromDot() {
    if (syncing()) return;
    setSyncing(true);
    try {
      await syncNow();
      await reloadWorkspace();
      setLastSynced(Date.now());
    } catch (e) {
      setAppState("lastError", e instanceof Error ? e.message : String(e));
    } finally {
      setSyncing(false);
    }
    await refresh();
  }

  function color(): string {
    const o = online();
    if (o === null) return "var(--color-outl-fg-dim)";
    // `accent_alt` is the palette's success hue and `warn` its warning
    // one — a hex here would be a second definition of a colour with no
    // `Palette` field behind it (root CLAUDE.md invariant 13).
    return o
      ? "var(--color-outl-accent-alt)"
      : "var(--color-outl-warn)";
  }

  function label(): string {
    if (syncing()) return "Syncing…";
    const o = online();
    if (o === null) return "Sync: checking…";
    return o ? "Sync: a peer is reachable" : "Sync: no peer reachable";
  }

  function tooltip(): string {
    const last = lastSynced();
    const when =
      last === null ? "Not synced yet this session" : `Last synced ${ago(now() - last)}`;
    if (syncing()) return `${label()}\n${when}`;
    return `${label()}\n${when}\nClick to sync now, right-click for Sync settings`;
  }

  return (
    <button
      type="button"
      aria-label={syncing() ? label() : `Sync now. ${label()}`}
      aria-busy={syncing()}
      title={tooltip()}
      onMouseEnter={() => setNow(Date.now())}
      onClick={() => void syncNowFromDot()}
      onContextMenu={(e) => {
        e.preventDefault();
        setAppState("settingsOpen", true);
      }}
      class="flex h-8 w-8 items-center justify-center rounded-full transition-colors hover:bg-(--color-outl-border)/40"
    >
      <Show
        when={syncing()}
        fallback={
          <span
            role="status"
            aria-live="polite"
            aria-hidden="true"
            class={`h-2.5 w-2.5 rounded-full${settling() ? " outl-sync-settle" : ""}`}
            style={{ background: color() }}
          />
        }
      >
        <span
          aria-hidden="true"
          data-testid="sync-spinner"
          class="h-2.5 w-2.5 animate-spin rounded-full border-2 border-(--color-outl-accent) border-t-transparent"
        />
      </Show>
    </button>
  );
}

/** "just now" / "3 min ago" / "2 h ago". Coarse on purpose: the tooltip
 *  answers "is this fresh?", not "at which second". */
export function ago(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}
