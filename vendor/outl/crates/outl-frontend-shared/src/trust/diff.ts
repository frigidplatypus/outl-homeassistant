/**
 * Word-level diff for the history panel.
 *
 * A block is a line or a short paragraph, so an O(n·m) LCS over words is
 * cheap and gives the reading people expect: the words that went away
 * struck through, the words that arrived highlighted, everything else
 * plain. Whitespace is kept as its own token so the rejoined segments
 * render the original spacing exactly.
 */

export type DiffOp = "same" | "added" | "removed";

export interface DiffSegment {
  op: DiffOp;
  text: string;
}

/** Above this many tokens per side the LCS table gets big; fall back to
 * "all removed, all added", which is still correct, just coarse. */
const MAX_TOKENS = 600;

function tokenize(text: string): string[] {
  return text.match(/\s+|[^\s]+/g) ?? [];
}

export function wordDiff(from: string, to: string): DiffSegment[] {
  const a = tokenize(from);
  const b = tokenize(to);
  if (a.length > MAX_TOKENS || b.length > MAX_TOKENS) {
    return merge([
      ...(from ? [{ op: "removed" as const, text: from }] : []),
      ...(to ? [{ op: "added" as const, text: to }] : []),
    ]);
  }
  // lcs[i][j] = LCS length of a[i..] and b[j..].
  const lcs: number[][] = Array.from({ length: a.length + 1 }, () =>
    new Array<number>(b.length + 1).fill(0),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: DiffSegment[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      out.push({ op: "same", text: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      out.push({ op: "removed", text: a[i++] });
    } else {
      out.push({ op: "added", text: b[j++] });
    }
  }
  while (i < a.length) out.push({ op: "removed", text: a[i++] });
  while (j < b.length) out.push({ op: "added", text: b[j++] });
  return merge(out);
}

/** Glue neighbours with the same op so the renderer emits few spans. */
function merge(segments: DiffSegment[]): DiffSegment[] {
  const out: DiffSegment[] = [];
  for (const s of segments) {
    const last = out[out.length - 1];
    if (last && last.op === s.op) last.text += s.text;
    else out.push({ ...s });
  }
  return out;
}
