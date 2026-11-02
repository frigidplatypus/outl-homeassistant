/**
 * Browser shim for `@tauri-apps/api/window`.
 *
 * The frontend calls `getCurrentWindow().close()` to quit the app. In a
 * browser the equivalent is closing the tab.
 */

export interface WebviewWindow {
  close(): Promise<void>;
  setTitle(title: string): Promise<void>;
  setFocus(): Promise<void>;
  isFocused(): Promise<boolean>;
  [key: string]: unknown;
}

export function getCurrentWindow(): WebviewWindow {
  return {
    close: async () => {
      if (typeof window !== "undefined") window.close();
    },
    setTitle: async (title: string) => {
      if (typeof document !== "undefined") document.title = title;
    },
    setFocus: async () => {
      if (typeof window !== "undefined") window.focus();
    },
    isFocused: async () => {
      return typeof document !== "undefined" && document.hasFocus();
    },
  };
}
