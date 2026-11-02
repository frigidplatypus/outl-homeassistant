/**
 * Browser shim for `@tauri-apps/plugin-log`.
 *
 * `attachConsole` forwards Rust `log::` lines into the webview console.
 * Outside a Tauri webview it is a no-op; the server logs to its own stdout,
 * which the addon captures.
 */

export async function attachConsole(): Promise<void> {
  // No-op in a plain browser.
}
