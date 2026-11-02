/** Renders a {@link wordDiff}: removed words struck, added ones tinted. */

import { For } from "solid-js";

import { wordDiff } from "./diff";

export function DiffText(props: { from: string; to: string; class?: string }) {
  return (
    <span class={`outl-diff ${props.class ?? ""}`}>
      <For each={wordDiff(props.from, props.to)}>
        {(seg) => (
          <span
            class={seg.op === "same" ? undefined : `outl-diff-${seg.op}`}
            aria-label={seg.op === "same" ? undefined : seg.op}
          >
            {seg.text}
          </span>
        )}
      </For>
    </span>
  );
}
