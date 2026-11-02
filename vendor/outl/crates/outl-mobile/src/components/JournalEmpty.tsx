import { Show } from "solid-js";
import { FirstRunTips } from "@outl/shared/polish";

/**
 * What the journal shows when there is no outline to show: still
 * loading, the workspace would not open, or the page has no blocks.
 *
 * Pure render. `Journal` decides which state it is in and owns every
 * action; this only lays the three states out, so the parent's JSX is
 * the outline and not three empty screens.
 */
export function JournalLoadFailed(props: { onRetry: () => void }) {
  return (
    <div class="flex flex-col items-center px-5 py-12 text-center">
      <p class="text-[15px] text-(--color-outl-fg-dim)">Couldn't open the workspace.</p>
      <button
        type="button"
        onClick={props.onRetry}
        class="mt-3 rounded-full bg-(--color-outl-accent) px-5 py-2 text-[14px] font-medium text-(--color-outl-accent-fg) active:opacity-70"
      >
        Retry
      </button>
    </div>
  );
}

/** The fresh-workspace state: nothing loaded yet, one tap starts writing. */
export function JournalStartWriting(props: { onStart: () => void }) {
  return (
    <>
    <div class="px-5 pt-6">
      <FirstRunTips client="mobile" onStart={props.onStart} />
    </div>
    <button
      type="button"
      onClick={props.onStart}
      class="flex w-full flex-col items-center px-5 py-16 text-center active:opacity-50"
    >
      <svg
        width="44"
        height="44"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="mb-3 text-(--color-outl-fg-dimmer)"
        aria-hidden="true"
      >
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z" />
      </svg>
      <p class="text-[15px] text-(--color-outl-fg-dim)">Nothing here yet.</p>
      <p class="mt-1 text-[13px] text-(--color-outl-accent)">Tap to start writing</p>
    </button>
    </>
  );
}

/**
 * An open page with no blocks. It is also the one state with no block
 * to long-press, so it carries the door to page properties as well.
 */
export function JournalEmptyPage(props: {
  journal: boolean;
  onAddBlock: () => void;
  onProperties: () => void;
}) {
  return (
    <div class="flex flex-col items-center gap-4 py-16 text-center">
      <div class="w-full px-5">
        <FirstRunTips client="mobile" onStart={props.onAddBlock} />
      </div>
      <p class="text-[15px] text-(--color-outl-fg-dim)">
        <Show when={props.journal} fallback="This page is empty">
          Nothing written this day
        </Show>
      </p>
      <div class="flex items-center gap-2">
        <button
          type="button"
          onClick={props.onAddBlock}
          class="rounded-full bg-(--color-outl-accent) px-4 py-2 text-[15px] font-medium text-(--color-outl-accent-fg) active:opacity-70"
        >
          Add a block
        </button>
        <button
          type="button"
          onClick={props.onProperties}
          class="rounded-full bg-(--color-outl-bg-elev) px-4 py-2 text-[15px] text-(--color-outl-fg) active:opacity-70"
        >
          Properties
        </button>
      </div>
      <p class="max-w-[16rem] text-[13px] text-(--color-outl-fg-dimmer)">
        Hold the title to edit page properties from anywhere.
      </p>
    </div>
  );
}
