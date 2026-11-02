/**
 * The devices sheet is mounted for the app's lifetime and gates its
 * markup on `open` — so every line of its *body* used to run at boot,
 * for a panel nobody had opened: `createSyncProgress()` subscribing to
 * `sync-progress` (whose handler calls `resolve_page_labels` per batch),
 * plus an `onMount` doing `hostname()` and two more `listen()`s.
 *
 * On iOS that is not free. `wry` serves Tauri IPC from the main thread
 * and a non-async command runs there, so work a closed sheet starts
 * competes with the journal's own first paint — the queue the app was
 * being SIGKILLed for (FRONTBOARD 0x8BADF00D).
 *
 * These pin the boundary in both directions: closed costs nothing, and
 * opening still wires everything up.
 */

import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const bridge = vi.hoisted(() => ({
  listen: vi.fn(async (_event: string, _cb?: unknown) => () => {}),
  hostname: vi.fn(async () => "phone"),
  peerList: vi.fn(async () => []),
  peerStatus: vi.fn(async () => []),
  deviceActivity: vi.fn(async () => []),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: bridge.listen }));
vi.mock("@tauri-apps/plugin-os", () => ({ hostname: bridge.hostname }));
vi.mock("@tauri-apps/plugin-barcode-scanner", () => ({
  scan: vi.fn(),
  cancel: vi.fn(async () => {}),
  checkPermissions: vi.fn(async () => "granted"),
  requestPermissions: vi.fn(async () => "granted"),
  Format: { QRCode: "QR_CODE" },
}));
vi.mock("@outl/shared/api/commands", () => ({
  peerList: bridge.peerList,
  peerStatus: bridge.peerStatus,
  peerRemove: vi.fn(),
  peerPairHost: vi.fn(),
  peerPairJoin: vi.fn(),
  resolvePageLabels: vi.fn(async () => []),
}));
vi.mock("@outl/shared/api/trust", () => ({
  deviceActivity: bridge.deviceActivity,
  exportMarkdown: vi.fn(),
  mergedEdits: vi.fn(async () => []),
  undoMergedEdit: vi.fn(),
}));
vi.mock("../lib/haptics", () => ({ haptic: () => {} }));

import { DevicesSheet } from "./DevicesSheet";

let dispose: (() => void) | undefined;

function mount(open: boolean) {
  const host = document.createElement("div");
  document.body.appendChild(host);
  dispose = render(() => <DevicesSheet open={open} onClose={() => {}} />, host);
}

const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  for (const fn of Object.values(bridge)) fn.mockClear();
  // Node 22's own `localStorage` global is `undefined` unless the runtime
  // was given `--localstorage-file`, and it shadows happy-dom's — so the
  // sheet's device-name read throws without a stub.
  vi.stubGlobal("localStorage", {
    getItem: () => null,
    setItem: () => {},
  });
});

afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = "";
});

describe("devices sheet", () => {
  it("a closed sheet reaches the backend for nothing", async () => {
    mount(false);
    await flush();

    expect(bridge.listen).not.toHaveBeenCalled();
    expect(bridge.hostname).not.toHaveBeenCalled();
    expect(bridge.peerList).not.toHaveBeenCalled();
    expect(bridge.deviceActivity).not.toHaveBeenCalled();
  });

  it("opening it wires the sync feed, the pairing events and the device list", async () => {
    mount(true);
    await flush();

    const events = bridge.listen.mock.calls.map((c) => c[0]);
    expect(events).toContain("sync-progress");
    expect(events).toContain("peer-paired");
    expect(events).toContain("peer-pair-failed");
    expect(bridge.hostname).toHaveBeenCalled();
    expect(bridge.peerList).toHaveBeenCalled();
    expect(bridge.deviceActivity).toHaveBeenCalled();
  });
});
