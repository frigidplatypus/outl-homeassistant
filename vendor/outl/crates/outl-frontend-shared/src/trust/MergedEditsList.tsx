/**
 * The sync panel's log of concurrent edits the CRDT merged, each with a
 * word diff (this device's text vs the merged result) and an Undo that
 * puts this device's text back as new ops.
 */

import { For, Show, createSignal, onMount } from "solid-js";

import type { MergedEdit } from "../api/trust";
import { deviceLabel, relativeAgo } from "./activity";
import { DiffText } from "./DiffText";
import { visibleMerges, withoutMerge } from "./merges";

export function MergedEditsList(props: {
  merges: MergedEdit[];
  onUndo: (id: string) => void;
  nowMs?: number;
}) {
  const rows = () => visibleMerges(props.merges);
  return (
    <div class="outl-merges">
      <Show when={rows().length > 0} fallback={<p class="outl-activity-note">No merged edits this session.</p>}>
        <For each={rows()}>
          {(m) => (
            <div class="outl-merge-row" data-merge-id={m.id}>
              <div class="outl-activity-meta">
                {m.page_title} · {deviceLabel({ actor: m.peer_actor, is_local: false, ops: 0, last_at_ms: 0 })} ·{" "}
                {relativeAgo(m.at_ms, props.nowMs)}
              </div>
              <DiffText class="outl-merge-diff" from={m.ours} to={m.merged} />
              <button type="button" class="outl-merge-undo" onClick={() => props.onUndo(m.id)}>
                Undo
              </button>
            </div>
          )}
        </For>
      </Show>
    </div>
  );
}

/**
 * The list wired to the backend: loads `merged_edits` when shown and
 * drops an entry once its undo lands. Both clients render this.
 */
export function MergedEditsPanel(props: { load: () => Promise<MergedEdit[]>; undo: (id: string) => Promise<unknown> }) {
  const [list, setList] = createSignal<MergedEdit[] | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  onMount(() => props.load().then(setList, (e) => setError(String(e))));
  const onUndo = (id: string) =>
    props.undo(id).then(
      () => setList((l) => withoutMerge(l ?? [], id)),
      (e) => setError(String(e)),
    );
  return (
    <Show when={list()} fallback={<p class="outl-activity-note">{error() ?? "Reading merged edits…"}</p>}>
      {(l) => <MergedEditsList merges={l()} onUndo={onUndo} />}
    </Show>
  );
}
