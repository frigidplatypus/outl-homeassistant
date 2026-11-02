import type { InlineToken } from "../api/types";

/**
 * A small stand-in for `outl_md::tokenize_owned`, good enough for the
 * fixture text the mock ships: `[[ref]]`, `#tag`, `**bold**` and
 * `` `code` ``. Real workspaces never go through here; the backend
 * tokenizes them.
 */
export function tokenize(text: string): InlineToken[] {
  const out: InlineToken[] = [];
  const re = /\[\[([^\]]+)\]\]|#([\w/-]+)|\*\*([^*]+)\*\*|`([^`]+)`/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    const at = m.index ?? 0;
    if (at > last) out.push({ kind: "plain", value: text.slice(last, at) });
    if (m[1] !== undefined) out.push({ kind: "ref", value: m[1] });
    else if (m[2] !== undefined) out.push({ kind: "tag", value: m[2] });
    else if (m[3] !== undefined) out.push({ kind: "bold", inner: tokenize(m[3]) });
    else out.push({ kind: "code", value: m[4] ?? "" });
    last = at + m[0].length;
  }
  if (last < text.length) out.push({ kind: "plain", value: text.slice(last) });
  return out;
}
