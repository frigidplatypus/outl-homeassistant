/**
 * The bottom of the Sync settings: what the op log holds from each
 * device, and the plain Markdown export. Presentation lives in
 * `@outl/shared/trust` so mobile's Devices sheet reads the same.
 */

import { Show, createResource } from "solid-js";

import { deviceActivity, exportMarkdown, mergedEdits, undoMergedEdit } from "@outl/shared/api/trust";
import { DeviceActivityList, ExportCard, MergedEditsPanel } from "@outl/shared/trust";

export function TrustSection() {
  const [activity] = createResource(deviceActivity);
  return (
    <section class="mt-4">
      <h4 class="outl-trust-heading">Changes in this workspace, by device</h4>
      <Show
        when={activity()}
        fallback={<p class="text-xs opacity-60">{activity.error ? String(activity.error) : "Reading the op log…"}</p>}
      >
        {(a) => <DeviceActivityList activity={a()} />}
      </Show>
      <h4 class="outl-trust-heading">Edits merged from other devices</h4>
      <MergedEditsPanel load={mergedEdits} undo={undoMergedEdit} />
      <h4 class="outl-trust-heading">Your notes</h4>
      <ExportCard onExport={() => exportMarkdown()} />
    </section>
  );
}
