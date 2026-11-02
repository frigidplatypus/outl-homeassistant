/**
 * Browser shim for `@tauri-apps/api/core`.
 *
 * The outl frontend calls `invoke(cmd, args)` to reach the Rust backend.
 * In a Tauri webview that is IPC; in a plain browser (the Home Assistant
 * addon) it is an HTTP POST to the outl-server. The server reuses the exact
 * same command bodies the desktop app uses, so the wire shape is identical.
 *
 * Tauri auto-converts camelCase JS keys to snake_case Rust params. The
 * frontend sends camelCase (`pageId`, `charOffset`), so we forward the args
 * as-is and let the server do the conversion.
 */

import { apiBase } from "./base";

const BASE: string = apiBase();
export async function invoke<T = unknown>(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const res = await fetch(`${BASE}/api/invoke/${encodeURIComponent(cmd)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    const msg =
      (body && (body.error || body.message)) ||
      `invoke ${cmd} failed: ${res.status}`;
    throw new Error(typeof msg === "string" ? msg : JSON.stringify(msg));
  }
  return body as T;
}

// Tauri also exposes `convertFileSrc` (asset:// URLs). The outl frontend
// does not use it, but provide a stub so any transitive import resolves.
export function convertFileSrc(_path: string, _protocol?: string): string {
  return "";
}
