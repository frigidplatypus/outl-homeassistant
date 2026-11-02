/**
 * "Your notes are yours": the one-click plain Markdown export. Pure:
 * the client passes the call, this owns the busy / done / failed states
 * so both apps say the same thing the same way.
 */

import { Show, createSignal } from "solid-js";

import type { ExportReport } from "../api/trust";

export function ExportCard(props: { onExport: () => Promise<ExportReport> }) {
  const [busy, setBusy] = createSignal(false);
  const [report, setReport] = createSignal<ExportReport | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      setReport(await props.onExport());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="outl-export">
      <p class="outl-export-lead">
        Your notes are plain Markdown files you own. Export copies every page, journal and
        attachment into a new folder next to your workspace, readable by any editor.
      </p>
      <button type="button" class="outl-export-btn" disabled={busy()} onClick={() => void run()}>
        {busy() ? "Exporting…" : "Export as Markdown"}
      </button>
      <Show when={report()}>
        {(r) => (
          <p class="outl-export-done" role="status">
            {r().pages} {r().pages === 1 ? "page" : "pages"}
            {r().assets ? ` and ${r().assets} attachments` : ""} copied to{" "}
            <code>{r().dir}</code>
          </p>
        )}
      </Show>
      <Show when={error()}>
        <p class="outl-export-error" role="alert">{error()}</p>
      </Show>
    </div>
  );
}
