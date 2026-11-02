import { Show, createEffect, createSignal } from "solid-js";

import type { PageView } from "@outl/shared/api/types";

import { captureToToday, closeQuickCapture, quickCaptureOpen } from "../lib/quick-capture";
import { appState } from "../lib/store";

const SAVED_FLASH_MS = 700;

/**
 * The quick capture box. One line, Enter saves to the end of today's
 * journal, Esc closes. Shift+Enter breaks the line for the rare
 * capture that needs two. See `lib/quick-capture.ts` for the doors
 * that open it.
 *
 * A failed write keeps the text in the box and says why, because a
 * capture that disappears on error is worse than no capture at all.
 */
export function QuickCapture(props: {
  applyView: (view: PageView) => void;
}) {
  const [text, setText] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [failure, setFailure] = createSignal<string | null>(null);
  /** Off today the write is invisible, so the box says so before closing. */
  const [saved, setSaved] = createSignal(false);
  let input: HTMLTextAreaElement | undefined;

  createEffect(() => {
    if (quickCaptureOpen()) queueMicrotask(() => input?.focus());
  });

  function close() {
    setFailure(null);
    setSaved(false);
    closeQuickCapture();
  }

  async function submit() {
    if (busy()) return;
    setBusy(true);
    try {
      const result = await captureToToday(text(), {
        currentPageId: () => appState.page?.id ?? null,
        applyView: props.applyView,
      });
      if (result === "empty") return;
      setText("");
      if (result === "shown") return close();
      setSaved(true);
      setTimeout(close, SAVED_FLASH_MS);
    } catch (e) {
      setFailure(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void submit();
    }
  }

  return (
    <Show when={quickCaptureOpen()}>
      <div
        class="fixed inset-0 z-50 flex items-start justify-center bg-black/50 backdrop-blur-sm"
        onClick={(e) => {
          if (e.target === e.currentTarget) close();
        }}
      >
        <div
          role="dialog"
          aria-label="Quick capture"
          class="mt-24 w-[520px] max-w-[90vw] overflow-hidden rounded-pop border border-(--color-outl-border)/60 bg-(--color-outl-bg-elev) shadow-pop"
        >
          <div class="px-4 pt-3 text-[11px] font-medium text-(--color-outl-fg-dimmer)">
            Add to today
          </div>
          <textarea
            ref={input}
            rows={2}
            value={text()}
            disabled={busy()}
            onInput={(e) => setText(e.currentTarget.value)}
            onKeyDown={onKey}
            placeholder="What's on your mind?"
            class="block w-full resize-none bg-transparent px-4 py-2 text-[15px] outline-none"
          />
          <Show when={failure()}>
            <div role="alert" class="px-4 pb-2 text-[12px] text-(--color-outl-destructive)">
              Not saved: {failure()}
            </div>
          </Show>
          <div class="border-t border-(--color-outl-fg)/10 px-4 py-1.5 text-[10px] opacity-50">
            {saved() ? "✓ Added to today's journal" : "⏎ save · ⇧⏎ new line · Esc close"}
          </div>
        </div>
      </div>
    </Show>
  );
}
