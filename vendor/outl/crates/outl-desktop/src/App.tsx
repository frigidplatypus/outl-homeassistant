import { Show, createSignal, onCleanup, onMount } from "solid-js";

import { getThemeConfig } from "@outl/shared/api/commands";
import { setDiagramsMode } from "@outl/shared/diagram";
import { installTheme } from "@outl/shared/theme";

import { currentWorkspace, getSettings, workspaceStats } from "./lib/api";
import { onWorkspaceReady } from "./lib/events";
import { setAppState } from "./lib/store";
import { AppShell } from "./components/AppShell";
import { Onboarding } from "./components/Onboarding";

/** `localStorage` key for the first-run flag (pure UI state — never an Op). */
const ONBOARDED_KEY = "outl.onboarded";

/**
 * Whether the user has completed (or skipped) first-run onboarding.
 *
 * This is a per-install UI flag, not workspace state, so it lives in
 * `localStorage` and deliberately does NOT go through the op log — it
 * must not converge across devices (each device onboards once). A
 * returning user whose workspace already opens at boot is treated as
 * onboarded so we never re-show the flow.
 */
function hasOnboarded(): boolean {
  try {
    return localStorage.getItem(ONBOARDED_KEY) === "1";
  } catch {
    return false;
  }
}

function markOnboarded() {
  try {
    localStorage.setItem(ONBOARDED_KEY, "1");
  } catch {
    // Private mode / disabled storage — onboarding just re-shows next
    // launch. Harmless, never blocks the app.
  }
}

/**
 * Root component.
 *
 * Three states, decided in order:
 *
 *   - workspace already open (returning user) → `<AppShell />`.
 *   - never onboarded → `<Onboarding />` (pick storage, optional pairing).
 *   - onboarded but no workspace open (e.g. picked folder removed) →
 *     fall back into `<Onboarding />` so the user re-picks.
 *
 * The split is intentional: the user can re-pick a workspace at runtime
 * (from a future "Switch workspace…" menu entry) without remounting the
 * whole app.
 */
function App() {
  const [ready, setReady] = createSignal(false);
  const [checked, setChecked] = createSignal(false);
  const [onboarded, setOnboarded] = createSignal(hasOnboarded());
  // Whether a workspace is already configured (`settings.last_workspace`).
  // Distinct from `ready` (the workspace has finished materializing): during
  // boot the config is known instantly but the background opener is still
  // replaying the op log, so `hasConfig && !ready` is the "opening" state — it
  // must render a loading screen, NOT the workspace picker. Showing the picker
  // there makes a returning user think their config was lost.
  const [hasConfig, setHasConfig] = createSignal(false);
  // Whether the backend has responded to at least one request. While
  // `false`, the gate cannot distinguish "no workspace configured" from
  // "server not up yet" — it must show a loading state, not the picker.
  const [serverReachable, setServerReachable] = createSignal(false);

  function finishOnboarding() {
    markOnboarded();
    setOnboarded(true);
  }

  async function refresh() {
    try {
      const ws = await currentWorkspace();
      setServerReachable(true);
      if (ws) {
        // A workspace is open, so one is configured. Set this here (not only
        // in `hydrateTheme`) so the gate is robust to a boot race: if the
        // first `getSettings()` call lands before the server is ready, the
        // polling `refresh()` still flips `hasConfig` once the workspace
        // opens, and the picker never appears.
        setHasConfig(true);
        const stats = await workspaceStats();
        setAppState("workspace", stats);
        setReady(stats.ready);
        // A returning user whose workspace already opens has, by
        // definition, set up before — never re-show onboarding for them.
        if (stats.ready && !onboarded()) finishOnboarding();
      } else {
        setReady(false);
        // The server is up but the workspace hasn't finished opening yet.
        // Check `last_workspace` from settings so the gate shows "Opening…"
        // instead of the picker during the boot window.
        try {
          const s = await getSettings();
          if (s.last_workspace != null) setHasConfig(true);
        } catch {
          // Server not reachable yet — leave hasConfig as-is.
        }
      }
    } catch {
      // The server is not reachable yet (the iframe can load before the
      // addon binds its port). Swallow the error so this `refresh()` resolves
      // instead of rejecting — a rejection would abort the `onMount` boot
      // sequence and skip the polling that recovers once the server is up.
      // State is left as-is; the next poll retries.
    } finally {
      setChecked(true);
    }
  }

  /**
   * Pull `[theme]` (RFC 0022's light/dark pair) via `get_theme_config`
   * and install it as CSS custom properties on `<html>` + `<body>`,
   * following `prefers-color-scheme` when `mode = "auto"`. Falls back
   * to the built-in `outl-light` / `outl` pair when settings/config
   * aren't readable yet — the default `_root_` CSS already paints the
   * brand background, so a delayed palette load just upgrades the
   * colors. Returns the `installTheme` unsubscribe so the caller can
   * tear down the media-query listener.
   */
  async function hydrateTheme(): Promise<() => void> {
    try {
      const s = await getSettings();
      setServerReachable(true);
      // A configured workspace is known from `settings.last_workspace` on disk
      // IMMEDIATELY — long before the background opener publishes the live
      // `storage_root` (that only happens after the op-log replay). This is the
      // signal the gate uses to show "opening…" instead of the picker, so read
      // it here, in the earliest boot step. `null` (or unreadable settings on a
      // true first run) leaves it false → the picker, correctly.
      setHasConfig(s.last_workspace != null);
      // Backlinks direction is a display preference read from the same
      // settings file (issue #142); hydrate it here so the first render
      // already orders correctly.
      if (s.backlinks_order === "oldest" || s.backlinks_order === "newest") {
        setAppState("backlinksOrder", s.backlinks_order);
      }
      setDiagramsMode(s.diagrams);
      const cfg = await getThemeConfig();
      return await installTheme({
        mode: cfg.mode,
        preset: cfg.preset,
        presetDark: cfg.preset_dark,
      });
    } catch {
      // First boot before the workspace lock is created — install the
      // built-in default pair explicitly so we still ship the brand
      // colors. A total backend failure keeps the static boot frame.
      try {
        return await installTheme({
          mode: "auto",
          preset: "outl-light",
          presetDark: "outl",
        });
      } catch {
        // No backend at all (shouldn't happen) — keep the static
        // boot frame.
        return () => {};
      }
    }
  }

  onMount(async () => {
    // Hydrate the theme BEFORE the rest of the boot routine so the
    // first painted frame already uses the user's chosen palette
    // — running it in parallel left a perceptible flash where
    // `refresh()` rendered with the static defaults from styles.css.
    const unsubscribeTheme = await hydrateTheme();
    onCleanup(unsubscribeTheme);
    await refresh();

    // Background opener (boot-time) emits `workspace-ready` when it
    // finishes opening `settings.last_workspace`. The picker flow
    // also emits the same event from `set_workspace`.
    const unlisten = await onWorkspaceReady(async () => {
      await refresh();
    });
    onCleanup(() => unlisten());

    // While the background opener is still in flight, poll every 500 ms so the
    // loading → shell transition isn't blocked on a missed `workspace-ready`
    // event. Cap at 40 attempts (20 s) — the first boot after a big sync can
    // full-replay for a few seconds before `ready`, and we must not fall back
    // to the picker while that's happening.
    if (!ready()) {
      let tries = 0;
      const id = setInterval(async () => {
        tries += 1;
        await refresh();
        if (ready() || tries >= 40) clearInterval(id);
      }, 500);
      onCleanup(() => clearInterval(id));
    }
  });

  return (
    <div class="h-full">
      <Show
        when={checked()}
        fallback={<div class="p-8 opacity-50">Loading…</div>}
      >
        {/* Ready + onboarded → the app. Not ready yet, but a workspace IS
            configured (or the server hasn't responded yet) → show a
            loading screen, never the picker. The picker only appears when
            the server is confirmed reachable AND reports no workspace —
            which in the addon never happens (the server always sets
            `last_workspace` at boot). */}
        <Show
          when={ready() && onboarded()}
          fallback={
            <Show
              when={hasConfig() || !serverReachable()}
              fallback={
                <Onboarding
                  onWorkspacePicked={refresh}
                  onFinish={finishOnboarding}
                />
              }
            >
              <div class="flex h-full items-center justify-center">
                <div class="flex items-center gap-3 opacity-60">
                  <div class="h-4 w-4 animate-spin rounded-full border-2 border-(--color-outl-fg)/30 border-t-(--color-outl-fg)/70" />
                  <span class="text-sm">Opening your workspace…</span>
                </div>
              </div>
            </Show>
          }
        >
          <AppShell />
        </Show>
      </Show>
    </div>
  );
}

export default App;
