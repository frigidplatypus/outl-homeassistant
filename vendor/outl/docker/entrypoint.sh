#!/bin/sh
# Entrypoint for the outl server image.
#
# Two jobs:
#
#   1. create the workspace if `serve` is what was asked for and none exists;
#   2. exec `outl`, so PID 1 is the daemon itself and `docker stop`'s SIGTERM
#      reaches the signal handler that releases the endpoint lease. A lease
#      left held locks every outl process on this device out of an endpoint.
#
# Everything after the entrypoint is passed to `outl` verbatim, so
# `docker run … peer pair --ticket X` and `docker run … doctor` work as-is.
#
# The image runs as uid 1000 and this script never runs as root, deliberately.
# A root entrypoint that chowns the volumes and then drops privileges is the
# more convenient shape, and it puts `docker exec` on the wrong side of the
# drop: exec bypasses the entrypoint, so `docker compose exec outl outl peer
# pair` — the documented way to add a device — would write `identity.key` and
# `peers.json` as root, and the daemon could no longer update them. Paying a
# one-time `chown` on a bind mount is the cheaper half of that trade.
set -eu

WORKSPACE="${OUTL_WORKSPACE:-/data}"

log() { printf '%s outl-entrypoint: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }

# Named volumes inherit the image's ownership and land here writable. A bind
# mount arrives owned by whoever owns it on the host, and the failure it
# produces further in is a confusing permission error from deep inside a
# reconcile — so check it here and name the fix.
for dir in "$WORKSPACE" "$HOME"; do
    if [ ! -w "$dir" ]; then
        log "FATAL: $dir is not writable by uid $(id -u)."
        log "If that path is a bind mount, take ownership on the HOST:"
        log "    sudo chown -R 1000:1000 <host path mounted at $dir>"
        log "Named volumes (what docker-compose.yml uses) need none of this."
        exit 1
    fi
done

mkdir -p "$HOME/.outl" "$XDG_CONFIG_HOME/outl"

# Only `serve` gets a workspace created for it. `peer pair` deliberately does
# not: it writes `.outl/workspace-id` itself, adopting the id of the graph it
# joins, and letting `init` run first would key this device's actor binding to
# an id that is about to be replaced. Pair first, then start the daemon — the
# order docs/self-hosting.md walks through.
if [ "${1:-}" = "serve" ] && [ ! -f "$WORKSPACE/.outl/config.toml" ]; then
    log "no workspace at $WORKSPACE — creating one with \`outl init --bare\`"
    log "(--bare writes no ops: a seeded replica would push a second"
    log "templates/journal page into the graph it pairs with)"
    outl init --bare "$WORKSPACE"
fi

exec outl --workspace "$WORKSPACE" "$@"
