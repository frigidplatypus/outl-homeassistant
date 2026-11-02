import { describe, expect, it } from "vitest";

import { htmlToOutlMarkdown } from "./html";

describe("htmlToOutlMarkdown", () => {
  it("converts bold to ** (outl strong delimiter)", () => {
    expect(htmlToOutlMarkdown("<b>hi</b>")).toBe("**hi**");
    expect(htmlToOutlMarkdown("<strong>hi</strong>")).toBe("**hi**");
  });

  it("converts italic to * — never _ (outl leaves intra-word _ literal)", () => {
    // The whole reason we override emDelimiter: outl does NOT treat
    // `_foo_` as emphasis, so emitting `_` here would round-trip wrong.
    expect(htmlToOutlMarkdown("<i>hi</i>")).toBe("*hi*");
    expect(htmlToOutlMarkdown("<em>hi</em>")).toBe("*hi*");
    expect(htmlToOutlMarkdown("<i>hi</i>")).not.toContain("_");
  });

  it("converts links to inline [text](url)", () => {
    expect(htmlToOutlMarkdown('<a href="https://x.test">site</a>')).toBe(
      "[site](https://x.test)",
    );
  });

  it("converts unordered lists to `- ` bullets (outl outline shape)", () => {
    const html = "<ul><li>one</li><li>two</li></ul>";
    expect(htmlToOutlMarkdown(html)).toBe("- one\n- two");
  });

  it("converts strikethrough to ~~ (Slack / GitHub)", () => {
    expect(htmlToOutlMarkdown("<del>gone</del>")).toBe("~~gone~~");
    expect(htmlToOutlMarkdown("<s>gone</s>")).toBe("~~gone~~");
    expect(htmlToOutlMarkdown("<strike>gone</strike>")).toBe("~~gone~~");
  });

  it("collapses inline images to their alt text (Slack custom emoji)", () => {
    // Slack renders `:bus:` as `<img alt=":bus:">`; we keep the shortcode
    // the outl renderer understands, not an `![](url)` image token.
    expect(htmlToOutlMarkdown('MarTech <img alt=":bus:" src="x.png">')).toBe(
      "MarTech :bus:",
    );
    // An image with no alt is dropped, not emitted as `![](url)`.
    expect(htmlToOutlMarkdown('<img src="x.png">')).toBe("");
  });

  it("converts inline code and fenced blocks", () => {
    expect(htmlToOutlMarkdown("<code>x</code>")).toBe("`x`");
    expect(htmlToOutlMarkdown("<pre><code>a\nb</code></pre>")).toContain("```");
  });

  it("returns empty string for blank / text-less HTML", () => {
    expect(htmlToOutlMarkdown("")).toBe("");
    expect(htmlToOutlMarkdown("   ")).toBe("");
    expect(htmlToOutlMarkdown("<div></div>")).toBe("");
  });

  it("converts a nested list keeping the child indented under a `- ` bullet", () => {
    // normalizeBullets exists for exactly this: the marker padding is
    // stripped to `- ` while the nesting indent is preserved (the backend
    // then folds 4-space nesting to 2-space).
    const html = "<ul><li>parent<ul><li>child</li></ul></li></ul>";
    const md = htmlToOutlMarkdown(html);
    expect(md).toMatch(/^- parent$/m);
    // child stays a bullet, indented (any leading whitespace) under parent.
    expect(md).toMatch(/^\s+- child$/m);
  });

  it("converts an ordered list to `1.` markers (normalizeBullets leaves them)", () => {
    const md = htmlToOutlMarkdown("<ol><li>one</li><li>two</li></ol>");
    expect(md).toContain("1. one");
    expect(md).toContain("2. two");
  });

  it("converts headings with atx `#`", () => {
    expect(htmlToOutlMarkdown("<h1>Title</h1>")).toBe("# Title");
    expect(htmlToOutlMarkdown("<h3>Sub</h3>")).toBe("### Sub");
  });

  it("converts blockquote to `> `", () => {
    expect(htmlToOutlMarkdown("<blockquote>quoted</blockquote>")).toBe(
      "> quoted",
    );
  });

  it("keeps the fence language from a code block class", () => {
    const html = '<pre><code class="language-rust">fn main() {}</code></pre>';
    const md = htmlToOutlMarkdown(html);
    expect(md).toContain("```rust");
    expect(md).toContain("fn main() {}");
  });

  it("keeps bold around a link (order of delimiters)", () => {
    const md = htmlToOutlMarkdown('<b><a href="https://x.test">site</a></b>');
    expect(md).toBe("**[site](https://x.test)**");
  });

  it("brings formatting across from a Slack-shaped message (the report)", () => {
    // Approximation of what Slack puts on `text/html`: bold "headers",
    // a custom emoji as <img alt>, and paragraphs. Before this, the paste
    // read text/plain and the bold was lost.
    const html =
      "<p><b>Marketing agora é MarTech</b> <img alt=\":bus:\" src=\"e.png\">, mais uma mudança</p>" +
      "<p><b>Por quê:</b><br>Marketing bom é feito de diagnóstico.</p>";
    const md = htmlToOutlMarkdown(html);
    expect(md).toContain("**Marketing agora é MarTech** :bus:, mais uma mudança");
    expect(md).toContain("**Por quê:**");
    expect(md).toContain("Marketing bom é feito de diagnóstico.");
    // Multi-paragraph → the backend splits it into one block per paragraph.
    expect(md.split(/\n{2,}/).length).toBeGreaterThanOrEqual(2);
  });

  it("decodes Google Docs inline font-weight (span 700 → bold)", () => {
    // Docs encodes bold as CSS weight on a span, not <b>. Without the
    // inline-weight rule this bold vanished.
    expect(
      htmlToOutlMarkdown('<span style="font-weight:700">bold</span> normal'),
    ).toBe("**bold** normal");
  });

  it("does NOT bold the whole block for the Docs `<b style=font-weight:normal>` wrapper", () => {
    // Docs wraps the entire payload in a non-bold <b>; only the inner
    // weighted span is truly bold.
    expect(
      htmlToOutlMarkdown(
        '<b style="font-weight:normal"><span style="font-weight:700">bold</span> plain</b>',
      ),
    ).toBe("**bold** plain");
  });

  it("handles a Notion-shaped paste (divs + <strong>)", () => {
    const md = htmlToOutlMarkdown(
      "<div>line one</div><div><strong>line two</strong></div>",
    );
    expect(md).toContain("line one");
    expect(md).toContain("**line two**");
    expect(md.split(/\n{2,}/).length).toBeGreaterThanOrEqual(2);
  });

  it("keeps paragraph breaks so multi-paragraph pastes split into blocks", () => {
    // Paragraphs come back blank-line separated → downstream
    // `hasMultipleParagraphs` / the backend split make one block each.
    const html = "<p><b>Header</b></p><p>Body line.</p>";
    const md = htmlToOutlMarkdown(html);
    expect(md).toContain("**Header**");
    expect(md).toContain("Body line.");
    expect(md.split(/\n{2,}/).length).toBeGreaterThanOrEqual(2);
  });
});
