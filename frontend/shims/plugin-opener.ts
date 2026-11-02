/**
 * Browser shim for `@tauri-apps/plugin-opener`.
 *
 * `openUrl` opens a link in the OS browser. In a plain browser the natural
 * equivalent is a new tab.
 */

export async function openUrl(href: string): Promise<void> {
  if (typeof window !== "undefined") {
    window.open(href, "_blank", "noopener,noreferrer");
  }
}
