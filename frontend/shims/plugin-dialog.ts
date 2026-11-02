/**
 * Browser shim for `@tauri-apps/plugin-dialog`.
 *
 * The desktop uses `open({ directory: true })` to let the user pick a
 * workspace folder. In the addon the workspace is a fixed mounted volume the
 * server opens at boot, so there is nothing to pick. `open` resolves to
 * `null` (the frontend treats a non-string as "cancelled") and the
 * WorkspacePicker never appears because `workspace_stats` already reports
 * ready.
 */

export interface OpenDialogOptions {
  directory?: boolean;
  multiple?: boolean;
  title?: string;
  [key: string]: unknown;
}

export async function open(_options?: OpenDialogOptions): Promise<string | string[] | null> {
  return null;
}

export async function save(_options?: OpenDialogOptions): Promise<string | null> {
  return null;
}

export async function message(_message: string, _options?: unknown): Promise<void> {
  // No native dialog surface in a browser; the UI shows errors inline.
}

export async function ask(_message: string, _options?: unknown): Promise<boolean> {
  return false;
}

export async function confirm(_message: string, _options?: unknown): Promise<boolean> {
  return false;
}
