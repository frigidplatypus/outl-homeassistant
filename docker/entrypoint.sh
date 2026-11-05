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

# `outl init` scaffolds the dirs but imports nothing, and the server's opener
# only replays the op log — pages dropped into `pages/` from the outside (an
# existing workspace copied in, a seeded fixture) stay invisible until a
# reconcile pass ingests them into ops. One-shot; cheap on a fresh workspace.
if [ -d "$WS/pages" ]; then
    if ! outl -w "$WS" serve --once >/dev/null 2>&1; then
        echo "WARNING: workspace reconcile failed; external pages may stay unimported" >&2
    fi
fi

# Pair into an existing sync cluster, if the add-on options carry a ticket.
# `pairing_ticket` is the Ticket: eyJ… string printed by `outl peer pair` on
# a machine that already holds the notes (host -> joiner; the joiner adopts
# the host's workspace-id and both folders become the same graph).
#
# This runs BEFORE outl-server starts on purpose: the server reads
# workspace-id into memory when it opens the workspace and never re-reads
# the file, so joining while it runs leaves it on the pre-adoption id and
# every exchange is refused as workspace-mismatch until restart. Boot-time
# joining is what keeps that invariant true without a reload dance.
#
# Idempotent: re-running with the same ticket re-adds the same peer entry.
# Failure is a warning, never a boot blocker — a stale ticket (the host's
# 120s pairing window has closed) after a successful earlier pair changes
# nothing; sync continues off peers.json. Clear the option once paired.
TICKET=""
if [ -f /data/options.json ]; then
    TICKET=$(sed -n 's/.*"pairing_ticket"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' /data/options.json 2>/dev/null || true)
fi

if [ -n "$TICKET" ]; then
    NAME=$(sed -n 's/.*"peer_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' /data/options.json 2>/dev/null || true)
    echo "joining sync cluster from pairing_ticket (workspace: $WS)"
    # `-` reads the ticket on stdin: tickets are 400-750 chars and belong
    # out of argv. `outl` resolves the workspace via -w (no env fallback).
    # shellcheck disable=SC2086
    if printf '%s' "$TICKET" | outl -w "$WS" peer pair --ticket - ${NAME:+--name "$NAME"}; then
        echo "paired; workspace-id adopted from the host"
    else
        echo "WARNING: pairing failed (stale ticket? host offline?); continuing unpaired" >&2
    fi
fi
exec "$@"
