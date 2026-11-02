/**
 * Compute the base path the outl-server is served under.
 *
 * In a Home Assistant ingress panel the addon UI loads in an iframe whose
 * document URL is `/hassio_ingress/{token}/`. The supervisor proxy strips
 * that prefix before forwarding to the addon, so the server always sees its
 * own root — but the browser resolves relative URLs against the iframe's
 * document URL, which keeps the prefix. So every API/WS call must be rooted
 * at the ingress path, not at the host root.
 *
 * Outside an ingress panel (standalone dev, `docker run -p 8090:8090`) the
 * path has no `/hassio_ingress/{token}/` segment, so the base is empty and
 * calls go to the origin root.
 *
 * `VITE_OUTL_API` (when set) overrides the auto-detection entirely, for
 * pointing the UI at a separate API host.
 */
export function apiBase(): string {
  const override = (import.meta as ImportMeta & { env?: { VITE_OUTL_API?: string } })
    .env?.VITE_OUTL_API;
  if (override) return override;
  const m = window.location.pathname.match(/^(\/hassio_ingress\/[^/]+)\//);
  return m ? m[1] : "";
}
