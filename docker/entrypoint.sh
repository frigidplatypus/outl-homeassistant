#!/bin/sh
# outl addon entrypoint.
#
# Ensures the workspace is initialized (an empty workspace dir becomes a
# fresh workspace) and then execs the requested command (default: outl-server).
#
# The workspace is the shared volume. If it is empty on first start, scaffold
# it so the server has something to open; an existing workspace is left
# untouched.
set -eu

WS="${OUTL_WORKSPACE:-/data/workspace}"

# Scaffold an empty workspace once. `outl init` is idempotent-safe: it only
# writes the ops/, journals/, pages/ scaffolding when absent.
if [ ! -d "$WS/ops" ] && [ ! -d "$WS/pages" ]; then
    echo "initializing empty workspace at $WS"
    if ! outl init "$WS"; then
        echo "WARNING: outl init failed; the server may open an empty or missing workspace" >&2
    fi
fi

exec "$@"
