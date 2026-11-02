import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";

import { ErrorToast } from "./ErrorToast";
import { appState, setAppState } from "../lib/store";

/**
 * The error surface moved from a bottom banner (covered by the fixed
 * ChromeToggleBar) to a top-right toast. These pin that it (a) only shows
 * when there's an error, (b) reacts to `lastError` changes, (c) is
 * dismissable, and (d) anchors in the notification corner above chrome —
 * so a refactor can't silently push it back under another element.
 */

let dispose: (() => void) | undefined;

function mount(): HTMLElement {
  const host = document.createElement("div");
  document.body.appendChild(host);
  dispose = render(() => <ErrorToast />, host);
  return host;
}

afterEach(() => {
  dispose?.();
  dispose = undefined;
  setAppState("lastError", null);
  document.body.innerHTML = "";
});

describe("ErrorToast", () => {
  it("renders nothing when there is no error", () => {
    setAppState("lastError", null);
    const host = mount();
    expect(host.textContent).toBe("");
    expect(
      document.querySelector('[aria-label="Dismiss error"]'),
    ).toBeNull();
  });

  it("shows the error message when lastError is set", () => {
    setAppState("lastError", "block X is not in the tree");
    mount();
    expect(document.body.textContent).toContain(
      "block X is not in the tree",
    );
  });

  it("reacts when lastError is set after mount", () => {
    setAppState("lastError", null);
    mount();
    expect(document.body.textContent).not.toContain("boom");
    setAppState("lastError", "boom");
    expect(document.body.textContent).toContain("boom");
  });

  it("dismiss button clears the error", () => {
    setAppState("lastError", "gone soon");
    mount();
    const btn = document.querySelector(
      '[aria-label="Dismiss error"]',
    ) as HTMLButtonElement | null;
    expect(btn).not.toBeNull();
    btn!.click();
    expect(appState.lastError).toBeNull();
    expect(document.body.textContent).not.toContain("gone soon");
  });

  it("anchors top-right above chrome (fixed, top-16 under the top bar, right-4, z-50)", () => {
    setAppState("lastError", "x");
    mount();
    const wrapper = document.querySelector(".fixed");
    expect(wrapper).not.toBeNull();
    const cls = wrapper!.className;
    expect(cls).toContain("top-16");
    expect(cls).toContain("right-4");
    expect(cls).toContain("z-50");
  });
});
