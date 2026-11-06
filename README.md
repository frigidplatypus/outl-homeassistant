# outl for Home Assistant

A Home Assistant addon that runs [outl](https://github.com/outlmd/outl) — a
local-first, markdown-backed outliner with CRDT P2P sync — as an always-on
peer. It serves the outl UI in a browser, exposes outl's MCP server to the
rest of the system, and shares the workspace volume with other processes.

Built on the browser-review work from
[outl PR #362](https://github.com/outlmd/outl/pull/362): the desktop client
runs in a plain browser, with the Tauri IPC seam redirected to an HTTP +
WebSocket backend that reuses the exact same command bodies the desktop app
uses.

## What it does

- **Browser UI.** The full outl desktop client (pages, journals, blocks,
  tasks, backlinks, search, themes, plugins) served at `http://<host>:8090/`.
  No native build — a real browser tab.
- **Sync.** Holds this device's iroh P2P endpoint so paired devices converge
  whether or not any two are awake at once. The workspace is a full replica;
  the op log is the source of truth. A file watcher on `ops/` also picks up
  peer writes that arrive through a shared filesystem.
- **MCP.** `outl mcp serve` runs as a child process and is bridged over HTTP
  at `POST /api/mcp` (JSON-RPC). Every outl CLI subcommand is available as an
  MCP tool to any MCP client on the network.
- **Shared filesystem.** The workspace lives in the Home Assistant
  configuration folder (`<config>/outl/`), the same pattern esphome uses to
  expose its device files. Other Home Assistant processes read the `.md`
  files directly on the host, or use the read-only HTTP API
  (`/api/fs/list`, `/api/fs/read`). Writes go through MCP or the `outl` CLI
  so the CRDT stays consistent.

## Layout

```
config.json            HA addon manifest
Dockerfile             multi-stage build (bun frontend + rust server + slim runtime)
docker/entrypoint.sh   scaffolds an empty workspace, then execs outl-server
frontend/              the browser build of the outl desktop client
  shims/               @tauri-apps/* -> HTTP/WebSocket shims
  vite.config.ts       aliases Tauri modules to the shims
vendor/outl/           outl source at a pinned commit
  crates/outl-server/  the HTTP host (AppHost impl + dispatch + events + fs + mcp)
```

## Build

```sh
# 1. Build the browser UI (needs bun or node)
cd frontend && bun install && bun run build   # -> frontend/dist

# 2. Build the addon image
cd .. && docker build -t outl-ha-addon .
```

The build compiles the whole outl workspace (iroh, wasmtime, tauri) because
`outl-server` reuses `outl-tauri-shared`'s command bodies. That is the slow
part; the BuildKit cache mounts make rebuilds incremental.

## Run (standalone, for development)

```sh
docker run -d --name outl \
  -p 8090:8090 \
  -v outl-data:/data \
  -v outl-config:/homeassistant \
  outl-ha-addon
```

Open `http://localhost:8090/`. Two mounts:

- `/homeassistant/outl` — the replicated notes, in the Home Assistant config
  folder. This is the user-visible workspace; edit the `.md` files directly
  on the host.
- `/data/home` — this device's identity (the iroh key + actor bindings).
  Private, never replicated. Lose it and the container comes back as a
  *new* device — unpaired, writing under a fresh actor.

## Install as a Home Assistant addon

1. Push this repo to a GitHub repo and add it as an addon repository in
   HA: **Settings → Add-ons → Add-on Store → ⋮ → Repositories**.
2. Set the `image` in `config.json` to your registry
   (`ghcr.io/<you>/outl-homeassistant-addon:{arch}`) and push the image.
3. Install the **outl** addon. It starts on boot and appears as a sidebar
   panel in the HA UI (no port is exposed — the panel is served through HA's
   ingress proxy, which authenticates the session). iroh reaches peers over
   the relay by default; direct hole-punching works when the host allows
   inbound UDP.

## Pairing a device

The addon joins an existing sync cluster through its configuration — there is
no shell step. Pairing runs at boot, before the server opens the workspace:
the server holds the workspace id in memory for its lifetime, so joining
while it runs would leave it on the pre-adoption id and every exchange would
be refused as a workspace mismatch until the next restart.

1. On the machine that already has the notes (laptop, phone — the *host*),
   start pairing. It prints a ticket and waits 120 seconds for the joiner:

   ```sh
   outl peer pair
   ```

2. Copy the `outlpair1.…` ticket into the addon's **Configuration →
   Pairing Ticket**, and optionally set **Peer Name** (the label the addon
   advertises; the hostname is the default).
3. Restart the addon within the 120-second window. It joins from the ticket,
   adopts the host's workspace id, and both folders become the same graph.
   Clear the ticket afterwards — it is single-use; replaying it logs a
   warning and boot continues unpaired (sync then runs off the peer list
   already on disk).

The two converge as soon as both are online. `outl peer list` on the host
shows the addon's status. Re-pairing is only needed after `outl peer revoke`
on the host, which rotates the workspace identity and locks out every device
— all of them, including ones you no longer have.

Identity lives on the addon's data volume, so pairing survives restarts and
addon updates; the workspace itself is the shared folder in the Home
Assistant config directory.

## Using the MCP server

Point any MCP client at the HTTP endpoint. The protocol is JSON-RPC 2.0,
newline-delimited over stdio in the native `outl mcp serve`; here it is one
request per HTTP POST. The `localhost:8090` examples below are the standalone
dev setup; in the HA addon the same endpoints are reached through the ingress
proxy (the panel's origin), not a published port.

```sh
curl -s http://localhost:8090/api/mcp \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

For an MCP client that speaks stdio, wrap the HTTP endpoint with any
stdio↔HTTP bridge (e.g. a small `mcp-remote`-style shim) — the JSON-RPC
payload is identical.

## Reading the workspace from other processes

The workspace is plain markdown in the Home Assistant config folder
(`<config>/outl/`). Read it directly on the host, or use the API:

```sh
# List the workspace root
curl -s 'http://localhost:8090/api/fs/list'

# Read a page
curl -s 'http://localhost:8090/api/fs/read?path=pages/My%20Page.md'
```

To *write* from another process, use the MCP tools (e.g. `outl_page_create`,
`outl_block_append`) or the `outl` CLI — both go through the op log. A raw
`.md` edit is also reconciled by the file watcher, but that is a fallback,
not the intended path.

## Environment

| Variable          | Default      | Meaning                                   |
|-------------------|--------------|-------------------------------------------|
| `OUTL_WORKSPACE`  | `/homeassistant/outl` | Workspace root (in the HA config folder) |
| `OUTL_PORT`       | `8090`       | HTTP port                                 |
| `OUTL_STATIC_DIR` | `/app/static`| Built frontend                            |
| `RUST_LOG`        | `info`       | Log filter (`outl=debug` to diagnose sync)|

## Notes and limits

- **Pairing is boot-time and option-driven.** Paste the ticket into
  **Configuration → Pairing Ticket**, restart within the 120-second window
  (see Pairing a device). The UI's pairing buttons are disabled: pairing
  must precede the server opening the workspace.
- **No runtime workspace swap.** The workspace is the fixed mounted volume,
  opened at boot. The "pick a folder" picker never appears.
- **OS file drag-and-drop** reports file names, not paths (a browser has no
  OS path); asset import by path still works through the API/MCP.
- **Reminders** are tracked and listed but not delivered as OS
  notifications (no notification daemon in the container).
