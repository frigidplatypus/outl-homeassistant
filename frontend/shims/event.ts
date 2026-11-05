/**
 * Browser shim for `@tauri-apps/api/event`.
 *
 * The outl frontend subscribes to backend events (`workspace-ready`,
 * `peer-ops-changed`, `workspace-reconciled`, `sync-progress`, …) via
 * `listen(name, handler)`. In a Tauri webview those are IPC events; in the
 * addon they arrive over a single WebSocket the outl-server holds open.
 *
 * The server pushes `{ event, payload }` frames. `listen` registers a local
 * handler and returns an `UnlistenFn` (a no-arg function that removes it),
 * matching the Tauri contract the frontend's `onCleanup` calls rely on.
 */

import { apiBase } from "./base";

type Handler = (event: { event: string; payload: unknown }) => void;

const handlers = new Map<string, Set<Handler>>();
let socket: WebSocket | null = null;

function wsUrl(): string {
  const base = apiBase();
  // Absolute URL required: `new WebSocket` rejects a relative path, and the
  // base is a path (`/api/hassio_ingress/{token}`) inside an ingress iframe.
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}${base}/api/events`;
}

function ensureSocket(): WebSocket {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return socket;
  }
  const next = new WebSocket(wsUrl());
  next.onmessage = (msg) => {
    let frame: { event: string; payload: unknown };
    try {
      frame = JSON.parse(msg.data as string);
    } catch {
      return;
    }
    const set = handlers.get(frame.event);
    if (!set) return;
    for (const h of [...set]) {
      try {
        h({ event: frame.event, payload: frame.payload });
      } catch (err) {
        console.error(`[outl] handler for ${frame.event} threw`, err);
      }
    }
  };
  next.onclose = () => {
    // Reconnect so a transient drop does not leave the UI stale.
    if (socket === next) {
      socket = null;
      setTimeout(() => ensureSocket(), 1500);
    }
  };
  socket = next;
  return next;
}

export function listen<T = unknown>(
  event: string,
  handler: (event: { event: string; payload: T }) => void,
): Promise<() => void> {
  ensureSocket();
  let set = handlers.get(event);
  if (!set) {
    set = new Set();
    handlers.set(event, set);
  }
  const wrapped: Handler = (e) => handler(e as { event: string; payload: T });
  set.add(wrapped);
  return Promise.resolve(() => {
    set.delete(wrapped);
  });
}

export type UnlistenFn = () => void;
