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

WS="${OUTL_WORKSPACE:-/homeassistant/outl}"

# The data mount is empty on first start. Create the device-identity dir so
# the iroh key + actor bindings have somewhere to live (iroh only syncs the
# workspace subtree, so this is never replicated).
mkdir -p "$HOME"

# The workspace lives in the Home Assistant config folder (mounted at
# /homeassistant). The mount exists but the outl/ subdir does not on first
# start, so create it before scaffolding.
mkdir -p "$WS"

# Scaffold an empty workspace once. `outl init` is idempotent-safe: it only
# writes the ops/, journals/, pages/ scaffolding when absent.
if [ ! -d "$WS/ops" ] && [ ! -d "$WS/pages" ]; then
    echo "initializing empty workspace at $WS"
    if ! outl init "$WS"; then
        echo "WARNING: outl init failed; the server may open an empty or missing workspace" >&2
    fi
fi

exec "$@"
