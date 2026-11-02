import { For } from "solid-js";

import { ChevronLeft } from "./JournalHeader";

/**
 * The bar above a zoomed outline: "Back" zooms out one level, and each
 * crumb jumps straight to that ancestor. Pure render; `Journal` owns
 * the zoom state.
 */
export function JournalZoomBar(props: {
  breadcrumb: { id: string; text: string }[];
  onZoomOut: () => void;
  onZoomTo: (id: string) => void;
}) {
  return (
    <div class="mb-1 flex items-center gap-1 overflow-x-auto px-4 pt-1 pb-2">
      <button
        type="button"
        aria-label="Zoom out"
        onClick={props.onZoomOut}
        class="flex shrink-0 items-center gap-1 rounded-full py-0.5 pr-2 pl-1 text-[13px] font-medium text-(--color-outl-accent) active:opacity-50"
      >
        <ChevronLeft />
        Back
      </button>
      <For each={props.breadcrumb}>
        {(crumb) => (
          <>
            <span aria-hidden="true" class="shrink-0 text-[12px] text-(--color-outl-fg-dimmer)">
              /
            </span>
            <button
              type="button"
              onClick={() => props.onZoomTo(crumb.id)}
              class="max-w-[12rem] shrink-0 truncate text-[13px] text-(--color-outl-fg-dim) active:opacity-50"
            >
              {crumb.text || "Untitled"}
            </button>
          </>
        )}
      </For>
    </div>
  );
}
