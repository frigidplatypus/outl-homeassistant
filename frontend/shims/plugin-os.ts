/**
 * Browser shim for `@tauri-apps/plugin-os`.
 *
 * `hostname` is used by the Sync panel to label this device. The browser
 * cannot read the OS hostname, so ask the server (which runs on the host)
 * for it.
 */

import { apiBase } from "./base";

const BASE = apiBase();

export async function hostname(): Promise<string> {
  try {
    const res = await fetch(`${BASE}/api/hostname`);
    if (res.ok) {
      const body = (await res.json()) as { hostname?: string };
      if (body.hostname) return body.hostname;
    }
  } catch {
    // fall through to the static fallback
  }
  return "home-assistant";
}
