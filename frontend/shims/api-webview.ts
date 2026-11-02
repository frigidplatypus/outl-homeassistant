/**
 * Browser shim for `@tauri-apps/api/webview`.
 *
 * The frontend uses `getCurrentWebview().onDragDropEvent` to receive OS
 * file drag-and-drop. A plain browser has no OS-level drag events, so this
 * wires the native HTML5 drag-and-drop API on `document` and maps it onto
 * the same callback shape.
 *
 * Limitation: a browser only exposes `File` objects, not filesystem paths.
 * The outl asset-import commands take a `sourcePath`, so a dropped file is
 * reported by name. Full OS-path drop is a Tauri-only capability; the
 * addon's primary paths (editing, sync, MCP, shared volume) do not depend
 * on it.
 */

type DragDropPayload =
  | { type: "enter"; position: { x: number; y: number } }
  | { type: "over"; position: { x: number; y: number } }
  | { type: "leave" }
  | { type: "drop"; position: { x: number; y: number }; paths: string[] };

type DragDropHandler = (event: { payload: DragDropPayload }) => void;

export interface Webview {
  onDragDropEvent(handler: DragDropHandler): Promise<() => void>;
  [key: string]: unknown;
}

export function getCurrentWebview(): Webview {
  return {
    onDragDropEvent(handler: DragDropHandler): Promise<() => void> {
      const dpr =
        typeof window !== "undefined" && window.devicePixelRatio
          ? window.devicePixelRatio
          : 1;

      const pos = (e: DragEvent) => ({ x: e.clientX * dpr, y: e.clientY * dpr });

      const onEnter = (e: DragEvent) => {
        e.preventDefault();
        handler({ payload: { type: "enter", position: pos(e) } });
      };
      const onOver = (e: DragEvent) => {
        e.preventDefault();
        handler({ payload: { type: "over", position: pos(e) } });
      };
      const onLeave = () => {
        handler({ payload: { type: "leave" } });
      };
      const onDrop = (e: DragEvent) => {
        e.preventDefault();
        const names = Array.from(e.dataTransfer?.files ?? []).map((f) => f.name);
        handler({ payload: { type: "drop", position: pos(e), paths: names } });
      };

      document.addEventListener("dragenter", onEnter);
      document.addEventListener("dragover", onOver);
      document.addEventListener("dragleave", onLeave);
      document.addEventListener("drop", onDrop);

      return Promise.resolve(() => {
        document.removeEventListener("dragenter", onEnter);
        document.removeEventListener("dragover", onOver);
        document.removeEventListener("dragleave", onLeave);
        document.removeEventListener("drop", onDrop);
      });
    },
  };
}
