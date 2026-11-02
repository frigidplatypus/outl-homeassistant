/**
 * Browser shim for `@tauri-apps/plugin-clipboard-manager`.
 *
 * The desktop reads the clipboard through the Tauri plugin because the macOS
 * WKWebview pops a native permission prompt for a programmatic
 * `navigator.clipboard` read. A real browser has no such prompt, so the web
 * Clipboard API is the correct backend here.
 */

export async function readText(): Promise<string> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.readText) {
    return navigator.clipboard.readText();
  }
  return "";
}

export async function writeText(text: string): Promise<void> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
  }
}
