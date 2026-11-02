import { For, Show, createMemo, createResource } from "solid-js";
import { DAY_LABELS, MONTH_NAMES, journalHeading, parseJournalSlug } from "@outl/shared/journal";
import { journalDaysWithContent } from "@outl/shared/api/overview";
import { ChevronDownIcon } from "@outl/shared/icons";

import { weekOf } from "../lib/week";
import { createDaySwipe } from "../lib/day-swipe";

/**
 * The week around the day on screen, under the journal header.
 *
 * One tap reaches any day this week; a dot marks the days that hold
 * text, so an empty week reads as empty without opening anything. The
 * month label opens the full calendar, and "Today" appears whenever the
 * strip is showing some other day.
 *
 * Swiping sideways here steps one day, the same as the header's
 * chevrons. The swipe stays on this strip and the header on purpose:
 * rows below swipe sideways for their own actions.
 */
export function WeekStrip(props: {
  selectedSlug: string;
  todaySlug: string | null;
  /** Changes whenever the open page's content may have; refetches the dots. */
  contentVersion: unknown;
  onPick: (slug: string) => void;
  onPrevDay: () => void;
  onNextDay: () => void;
  onToday: () => void;
  onOpenCalendar: () => void;
}) {
  const week = createMemo(() => weekOf(props.selectedSlug));
  const [withContent] = createResource(
    () => {
      const w = week();
      return w.length ? { from: w[0], to: w[6], v: props.contentVersion } : null;
    },
    ({ from, to }) => journalDaysWithContent(from, to).catch(() => [] as string[]),
  );
  const filled = createMemo(() => new Set(withContent() ?? []));
  const month = () => {
    const p = parseJournalSlug(props.selectedSlug);
    return p ? `${MONTH_NAMES[p.monthIndex]} ${p.year}` : "";
  };
  const swipe = createDaySwipe({ prev: props.onPrevDay, next: props.onNextDay });

  return (
    <div
      class="px-3 pt-1"
      onTouchStart={swipe.onTouchStart}
      onTouchEnd={swipe.onTouchEnd}
      onTouchCancel={swipe.onTouchCancel}
    >
      <div class="flex items-center justify-between px-1 pb-1">
        <button
          type="button"
          onClick={props.onOpenCalendar}
          aria-label={`Open the calendar, showing ${month()}`}
          class="flex items-center gap-1 rounded-full py-1 font-medium text-(--color-outl-fg-dim) active:opacity-60"
        >
          <span class="text-[13px]">{month()}</span>
          <ChevronDownIcon size={13} />
        </button>
        <Show when={props.todaySlug && props.todaySlug !== props.selectedSlug}>
          <button
            type="button"
            onClick={props.onToday}
            class="rounded-full bg-(--color-outl-accent)/12 px-2.5 py-1 font-medium text-(--color-outl-accent) active:opacity-60"
          >
            <span class="text-[12px]">Today</span>
          </button>
        </Show>
      </div>
      <div class="grid grid-cols-7 gap-1" role="group" aria-label="This week">
        <For each={week()}>
          {(slug, i) => {
            const selected = () => slug === props.selectedSlug;
            const today = () => slug === props.todaySlug;
            return (
              <button
                type="button"
                onClick={() => props.onPick(slug)}
                aria-pressed={selected()}
                aria-label={`${journalHeading(slug, props.todaySlug)?.date ?? slug}${filled().has(slug) ? ", has entries" : ""}`}
                class="flex flex-col items-center gap-0.5 rounded-xl py-1.5 active:opacity-60"
                classList={{
                  "bg-(--color-outl-accent) text-(--color-outl-accent-fg)":
                    selected(),
                }}
              >
                {/*
                  Both spans name `accent_fg` explicitly on the selected
                  path instead of inheriting it from the button. They
                  already set their own colour when NOT selected, so the
                  selected path was the only one leaning on inheritance —
                  and that inheritance used not to arrive: `styles.css`
                  carried `button { color: inherit }` outside any `@layer`,
                  and an unlayered rule beats every Tailwind utility
                  regardless of specificity, so a text colour set on a
                  <button> was dead on this client. That line is gone, but
                  a colour that has to travel through a parent to be right
                  is a colour that can be wrong again.
                */}
                <span
                  class="text-[11px] font-medium"
                  classList={{
                    "text-(--color-outl-accent-fg)": selected(),
                    "text-(--color-outl-fg-dimmer)": !selected(),
                  }}
                >
                  {DAY_LABELS[i()]}
                </span>
                <span
                  class="text-[16px] font-semibold tabular-nums leading-none"
                  classList={{
                    "text-(--color-outl-accent-fg)": selected(),
                    "text-(--color-outl-accent)": today() && !selected(),
                  }}
                >
                  {Number(slug.slice(8))}
                </span>
                <span
                  aria-hidden="true"
                  class="h-1 w-1 rounded-full"
                  classList={{
                    "bg-(--color-outl-accent)": filled().has(slug) && !selected(),
                    "bg-(--color-outl-accent-fg)":
                      filled().has(slug) && selected(),
                    "bg-transparent": !filled().has(slug),
                  }}
                />
              </button>
            );
          }}
        </For>
      </div>
    </div>
  );
}
