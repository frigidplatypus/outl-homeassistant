import { formatJournalSlug, journalSlugToDate } from "@outl/shared/journal";

/**
 * The seven journal slugs of the week holding `slug`, Sunday first,
 * the same week shape the month calendar draws. Empty for a slug that
 * is not a date, so a page that is not a journal gets no strip.
 */
export function weekOf(slug: string | null | undefined): string[] {
  const day = journalSlugToDate(slug);
  if (!day) return [];
  const sunday = new Date(day.getFullYear(), day.getMonth(), day.getDate() - day.getDay());
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(sunday.getFullYear(), sunday.getMonth(), sunday.getDate() + i);
    return formatJournalSlug(d.getFullYear(), d.getMonth(), d.getDate());
  });
}
