/**
 * Who is allowed to put the journal back behind the skeleton, and what
 * that gate is allowed to enclose.
 *
 * The complaint was "opening the app is slow to load today's journal",
 * and the half of it that is not ordering is this: `loaded` is the sole
 * gate on `<SkeletonOutline />`, so whatever it encloses disappears
 * whenever it is cleared. `UX.md` § 2, never block a keystroke, draws
 * the line — work landing after the first paint shows its own
 * placeholder in its own section and never unmounts, disables or re-keys
 * the editor — and the desktop already regressed across it once,
 * carrying a page-level load state for an `O(blocks-in-workspace)`
 * backlinks scan.
 *
 * Three facts are pinned here, each of which was a real defect in one
 * client or the other:
 *
 * 1. **The gate encloses the outline, not the page.** The header, the
 *    week strip, the page-property chips, the backlinks card and the
 *    bottom bar are outside it, so a slow open cannot take them with it.
 * 2. **Only the boot load clears it, and only with nothing on screen.**
 *    The retry button used to call `setLoaded(false)` unconditionally,
 *    which unmounts the outline — and the caret and the user's
 *    keystrokes — wherever that state becomes reachable with a page up.
 * 3. **Day navigation cannot reach it at all.** Moving between days
 *    swaps the applied view and leaves the previous day rendered until
 *    the next one arrives; `Journal.day-nav.ts` is handed no gate setter
 *    and must stay that way.
 *
 * Read as source, not rendered: mounting `Journal` needs haptics, the
 * Tauri bridge and a WKWebView-shaped textarea, so a test that mocks all
 * three would pin the mocks — the same reasoning (and the same shape) as
 * `BlockRow.recursion.test.ts`.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const JOURNAL = readFileSync(join(import.meta.dirname, "Journal.tsx"), "utf8");
const DAY_NAV = readFileSync(
  join(import.meta.dirname, "Journal.day-nav.ts"),
  "utf8",
);

/** Every index at which `re` matches. */
function indices(source: string, re: RegExp): number[] {
  return [...source.matchAll(re)].map((m) => m.index ?? -1);
}

function only(source: string, re: RegExp, what: string): number {
  const found = indices(source, re);
  expect(found, `expected exactly one ${what}`).toHaveLength(1);
  return found[0]!;
}

/** The `<section>` the skeleton stands in for, as [start, end). */
function outlineSection(): [number, number] {
  const skeleton = only(JOURNAL, /<SkeletonOutline\b/g, "<SkeletonOutline />");
  const start = indices(JOURNAL, /<section\b/g)
    .filter((i) => i < skeleton)
    .pop();
  const end = indices(JOURNAL, /<\/section>/g).find((i) => i > skeleton);
  expect(start, "the skeleton must stand inside a <section>").toBeDefined();
  expect(end, "that <section> must be closed").toBeDefined();
  return [start!, end!];
}

describe("the load gate", () => {
  it("encloses the outline section, not the page", () => {
    const [start, end] = outlineSection();

    for (const at of indices(JOURNAL, /\bloaded\(\)/g)) {
      expect(
        at > start && at < end,
        "every `loaded()` read belongs inside the outline section — a read " +
          "outside it puts chrome the user could already be using behind " +
          "one command's reply",
      ).toBe(true);
    }
  });

  it("leaves the chrome, the week strip and the sections below outside it", () => {
    const [start, end] = outlineSection();

    for (const tag of ["<JournalChrome", "<WeekStrip"]) {
      const at = only(JOURNAL, new RegExp(tag + "\\b", "g"), tag);
      expect(at, `${tag} must render ahead of the gated section`).toBeLessThan(
        start,
      );
    }
    // Backlinks and nested pages land after the first paint too, and each
    // carries its own placeholder — `<PageSections />` is a sibling of the
    // outline, never inside its gate.
    const sections = only(JOURNAL, /<PageSections\b/g, "<PageSections");
    expect(sections).toBeGreaterThan(end);
  });

  it("is cleared only with nothing on screen to lose", () => {
    const clears = indices(JOURNAL, /setLoaded\(false\)/g);
    expect(clears, "one retry path, no other way back to the skeleton").toHaveLength(
      1,
    );
    expect(
      /if \(!view\(\)\)\s*\{?\s*setLoaded\(false\)/.test(JOURNAL),
      "a retry taken with a page up must reload behind what is already " +
        "rendered, not unmount it",
    ).toBe(true);
  });

  it("cannot be reached by day navigation", () => {
    // Every day change — the header chevrons, the week strip, the
    // calendar, the bottom bar's Journal tab, wake-to-today — runs
    // through this module, and its whole dependency surface is view /
    // editingId / applyView / withError. Handing it a gate setter is how
    // "go to yesterday" becomes a blank page with a shimmer on it.
    expect(DAY_NAV).not.toMatch(/setLoaded|setLoadFailed|setView\(/);
    expect(DAY_NAV).toMatch(/applyView/);
  });
});
