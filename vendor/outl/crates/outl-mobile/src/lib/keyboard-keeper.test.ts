import { afterEach, describe, expect, it } from "vitest";

import {
  armKeyboard,
  installKeyboardKeeper,
  releaseKeyboard,
  takeArmedText,
} from "./keyboard-keeper";

/**
 * **These tests are a proxy, and only a device proves the feature.**
 * jsdom / happy-dom has no software keyboard, so nothing here can
 * assert that iOS raised one. What they pin is the condition iOS
 * actually keys off and which a refactor can silently break: that a
 * *real, laid-out* text input holds focus, put there synchronously
 * inside the tap handler. Everything else is the platform's half of
 * the deal.
 */

afterEach(() => {
  releaseKeyboard();
  document.body.innerHTML = "";
});

function keeperEl(): HTMLTextAreaElement {
  const el = document.body.querySelector("textarea[aria-hidden='true']");
  if (!el) throw new Error("keeper not installed");
  return el as HTMLTextAreaElement;
}

describe("installKeyboardKeeper", () => {
  it("appends one real textarea to the body", () => {
    installKeyboardKeeper();
    expect(document.body.querySelectorAll("textarea").length).toBe(1);
    expect(keeperEl().tagName).toBe("TEXTAREA");
  });

  it("is idempotent", () => {
    installKeyboardKeeper();
    installKeyboardKeeper();
    installKeyboardKeeper();
    expect(document.body.querySelectorAll("textarea").length).toBe(1);
  });

  it("re-installs after the node is removed from the document", () => {
    installKeyboardKeeper();
    keeperEl().remove();
    installKeyboardKeeper();
    expect(document.body.querySelectorAll("textarea").length).toBe(1);
  });

  it("stays laid out, because iOS raises no keyboard for a hidden field", () => {
    const style = installKeyboardKeeper()!.getAttribute("style") ?? "";
    expect(style).not.toMatch(/display\s*:\s*none/);
    expect(style).not.toMatch(/visibility\s*:\s*hidden/);
    // Below 16px, focusing an input makes iOS zoom the page.
    expect(style).toMatch(/font-size\s*:\s*16px/);
    const el = keeperEl();
    expect(el.disabled).toBe(false);
    expect(el.readOnly).toBe(false);
  });
});

describe("armKeyboard", () => {
  it("focuses the keeper synchronously", () => {
    expect(armKeyboard()).toBe(true);
    expect(document.activeElement).toBe(keeperEl());
  });

  it("installs the keeper itself when nothing did", () => {
    armKeyboard();
    expect(document.activeElement?.tagName).toBe("TEXTAREA");
  });

  it("leaves an already-focused text input alone", () => {
    const ta = document.createElement("textarea");
    document.body.appendChild(ta);
    ta.focus();
    // The keyboard is already up for that field; stealing focus would
    // blur it, which commits the draft underneath the user.
    expect(armKeyboard()).toBe(false);
    expect(document.activeElement).toBe(ta);
  });

  it("leaves an already-focused input alone too", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    expect(armKeyboard()).toBe(false);
    expect(document.activeElement).toBe(input);
  });

  it("clears stale text so nothing leaks into the next block", () => {
    armKeyboard();
    keeperEl().value = "never drained";
    keeperEl().blur();
    armKeyboard();
    expect(keeperEl().value).toBe("");
  });
});

describe("takeArmedText", () => {
  it("returns what was typed while the keeper held focus, and drains it", () => {
    armKeyboard();
    keeperEl().value = "ab";
    expect(takeArmedText()).toBe("ab");
    expect(takeArmedText()).toBe("");
  });

  it("is empty when nothing was typed", () => {
    armKeyboard();
    expect(takeArmedText()).toBe("");
  });
});

describe("releaseKeyboard", () => {
  it("drops focus so the user is not typing into an invisible field", () => {
    armKeyboard();
    releaseKeyboard();
    expect(document.activeElement).not.toBe(keeperEl());
  });

  it("does not touch a textarea that is not the keeper", () => {
    installKeyboardKeeper();
    const ta = document.createElement("textarea");
    document.body.appendChild(ta);
    ta.focus();
    releaseKeyboard();
    expect(document.activeElement).toBe(ta);
  });
});
