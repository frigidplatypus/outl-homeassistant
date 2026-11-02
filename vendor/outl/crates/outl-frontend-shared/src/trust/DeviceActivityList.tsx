/**
 * The honest half of the sync panel: what the op log holds from each
 * device. Pure: data in, rows out.
 */

import { For, Show } from "solid-js";

import type { DeviceActivity } from "../api/trust";
import { deviceLabel, relativeAgo } from "./activity";

export function DeviceActivityList(props: { activity: DeviceActivity; nowMs?: number }) {
  return (
    <div class="outl-activity">
      <For each={props.activity.actors}>
        {(a) => (
          <div class="outl-activity-row" data-local={a.is_local ? "" : undefined}>
            <span class="outl-activity-name">{deviceLabel(a)}</span>
            <span class="outl-activity-meta">
              {a.ops} {a.ops === 1 ? "change" : "changes"} · last {relativeAgo(a.last_at_ms, props.nowMs)}
            </span>
          </div>
        )}
      </For>
      <Show when={!props.activity.complete}>
        <p class="outl-activity-note">Counts since the last snapshot on this device.</p>
      </Show>
    </div>
  );
}
