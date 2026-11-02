import { render } from "solid-js/web";
import { describe, expect, it } from "vitest";
import { PageGlyph } from "./index";

function mount(el: () => import("solid-js").JSX.Element) {
  const host = document.createElement("div");
  render(el, host);
  return host;
}

describe("PageGlyph", () => {
  it("prefers the page's own icon", () => {
    const host = mount(() => <PageGlyph icon="🚀" kind="page" />);
    expect(host.textContent).toBe("🚀");
    expect(host.querySelector("svg")).toBeNull();
  });

  it("falls back to a line icon, not an emoji", () => {
    const host = mount(() => <PageGlyph kind="journal" />);
    expect(host.querySelector("svg")).not.toBeNull();
    expect(host.textContent).toBe("");
  });

  it("is decorative unless titled", () => {
    const host = mount(() => <PageGlyph kind="page" />);
    expect(host.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
  });
});
