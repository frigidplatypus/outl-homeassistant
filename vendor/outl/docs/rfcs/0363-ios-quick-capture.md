# RFC 0363: Capture from outside the app lands through an inbox the app drains

| | |
|---|---|
| **Status** | Accepted (2026-10-12) |
| **Issue** | [#363](https://github.com/outlmd/outl/issues/363) |
| **PR** | [#362](https://github.com/outlmd/outl/pull/362) carries this RFC and its implementation (accepted 2026-10-12, implemented in the same PR) |
| **Date** | 2026-10-12 |
| **Reference doc** | none yet; `UX.md` "Getting around on a phone" covers the in-app capture this extends |
| **Invariant** | root `CLAUDE.md` invariant 7 (state that must converge goes through the op log) |
| **Guarded by** | `outl_actions::capture` tests (the four below plus `a_malformed_capture_is_kept_and_counted`, `the_block_id_is_the_capture_ulid`, `a_share_becomes_one_line` and the `write_capture` round-trip set), `outl-tauri-shared/tests/capture_inbox_command.rs`, OutlKit `CaptureInboxTests` |

## Why

The phone's capture button (#362) writes to today's journal in one tap, but only once the app is open on screen.
The entry points iOS users reach for first live outside the app: a Home Screen or Lock Screen widget, a Shortcut that Siri and the Action button can call with text, and the share sheet.
Each of those runs in its own process, with its own sandbox and a memory ceiling far below what booting a workspace needs.
None of them can call `outl_actions` the way the app does, and none of them can see the workspace while it lives in the app's own container.

A capture path that loses a line is worse than no capture path, because the user stops trusting the journal.
So the hard part is not the widget, it is who turns the captured text into an op, and when.

## What we chose

Extensions never touch the workspace.
They append one record per capture to an inbox in the App Group container, and the app drains it.

- **Inbox.** `captures/` in the App Group container, one file per capture, named by a ULID so the names sort in capture order and two writers never collide.
  The file holds the text, the capture time and the source (`intent` or `share` from iOS; `share` from Android, `cli` from `outl capture` — see the amendment below).
  The reader takes `source` as a plain string and never matches on it, so a writer on a newer build can add a value without the file being refused.
  Writing is create-temp, fsync, rename, so the app either sees a whole capture or none of it.
- **Drain.** The app drains on launch, on every return to the foreground, and on a Darwin notification the extension posts after its rename, so a capture made while the app is open lands without a reload. Draining goes through `outl_actions` like any typed block: it opens or creates the journal for the capture's own day and appends one block per file, in name order. The file is deleted only after the op is durable in the op log.
- **Idempotency.** The block id is derived from the capture's ULID, so a drain that crashes between the op and the delete replays to the same block instead of a second copy.
- **Owner.** One function, `drain_capture_inbox`, in the mobile backend. The extensions share a small Swift writer and nothing else.

The rollout order follows the risk:

1. The widget opens `outl://capture`, which lands in the capture sheet. No cross-process write, so it can ship before the inbox does. The link itself landed in #362 (`DeepLinkTarget::Capture`); the widget that opens it did not.
2. The App Intent "Add to today", which also covers Shortcuts, Siri and the Action button.
3. The share extension, writing text or a URL through the same inbox.

Moving the workspace into the App Group container is not part of this. The inbox is the only thing the extensions share with the app.

## Why not the alternatives

**Extensions append to today's `.md` directly.** That is the write the reconciler treats as an external edit, and on a phone it races the app's own projection of the same file. It also hands the day-file naming and the markdown dialect to Swift, a second owner of a format that has one.

**Extensions write ops into the op log.** It keeps invariant 7 on paper, but needs the workspace in the App Group (a migration on every installed copy), the actor id and the HLC in a second process, and a second writer to an append-only log that today assumes one. The memory ceiling makes it fragile as well.

**Open the app for every capture.** That is the widget step above, and it is fine as a first step. It does nothing for Shortcuts or the share sheet, where leaving the current app is the cost we are trying to remove.

## The opposite direction

The app now writes on behalf of something that happened earlier and elsewhere.
A capture made at 23:58 and drained at 00:10 belongs to the day it was made, so the drain uses the capture time, not the drain time, to pick the journal.
That means a drain can create a past day's journal page, which opening the app never does on its own.

Ordering against the user's own typing is by drain time, not capture time: a capture made on the lock screen while the app was open in the background appears below what the user typed after it.
That is the honest order of what the app saw, and the block keeps its capture time.

If the app is never opened, captures wait in the inbox indefinitely and do not sync. The widget and the intent say "Saved, opens in outl" rather than implying the line is already on every device.

## How it cannot regress

1. **The invariant.** Root `CLAUDE.md` invariant 7, plus a line in `crates/outl-mobile/CLAUDE.md` naming `drain_capture_inbox` as the only path from the inbox to the workspace.
2. **The tests.**
   - `drain_appends_in_capture_order`: three files with out-of-order mtimes land in ULID order.
   - `drain_is_idempotent_after_a_crash`: an op committed with its file left behind replays to one block, not two.
   - `capture_lands_in_the_day_it_was_made`: a capture stamped yesterday lands in yesterday's journal.
   - `partial_file_is_ignored`: a temp file with no rename is skipped and left for the next drain.

## Implementation and what needs a Mac

Landed in #362: the widget (`OutlWidget`, iOS 16+), the inbox writer in OutlKit, `drain_capture_inbox`, the App Intent "Add to today" with its App Shortcut (in the app, iOS 16+, `AppIntents` weak-linked), and the share extension (`OutlShare`, iOS 15+).
CI covers the pure Swift (`swift test`), the Rust drain, the webview drain hooks, and an unsigned iOS Simulator build that compiles and embeds both extensions.

Signing and device behaviour cannot be checked without Xcode and the Apple account:

- **App Group** `group.app.outl.mobile-app`, enabled on `app.outl.mobile-app` and `app.outl.mobile-app.share`.
- **App IDs** `app.outl.mobile-app.widget` and `app.outl.mobile-app.share`.
- **Profiles**: regenerate the app's App Store profile with the App Group, plus one App Store profile per extension, stored as `WIDGET_PROVISIONING_PROFILE_BASE64` and `SHARE_PROVISIONING_PROFILE_BASE64`. Until both exist, the release job unhooks the extensions and drops the App Group, so it keeps shipping the app alone.
- **Device run**: widget on the Home and Lock Screen opens the capture sheet; the Shortcut and the share sheet save a line that lands in today's journal on the next open, or within a second when the app is already open.

The step-by-step list is `crates/outl-mobile/IOS-EXTENSIONS.md`.

## Amendment (2026-10-16): Android and the desktop

Accepted as above and extended, in the same issue, because both turned out to be the same hand-off with a different writer.

**Android** gets the three shapes this RFC lists — a share target, a home screen widget and a quick settings tile — against the same inbox, at `filesDir/captures` (exported as `OUTL_CAPTURE_INBOX`, exactly as the iOS shell does it). Two differences, both forced by the platform rather than chosen:

- **The format has one owner there, in Rust.** Android has no extension model: the share target is an activity in *this* app's process with the library loaded, so it writes through `Java_app_outl_mobile_1app_OutlCapture_write` → `write_capture`. The Swift writer exists only because an iOS extension cannot load the library. So "a share becomes one line" also moved into Rust (`shared_line`) with `SharedLine.swift` as its mirror, rather than gaining a third copy in Kotlin.
- **No arrival notification, and that is not an omission.** iOS needs a Darwin notification because an iOS extension can write while the app is on screen. On Android a share sheet belongs to the app the user is *in*, and the widget and tile open the app rather than writing, so coming back is the only moment a capture can be new — `MainActivity.onResume` nudges the webview instead.

**The desktop and the TUI** get the inbox too, as `<root>/.outl/captures/`, written by `outl capture`. That is a second inbox location, and the two answer different questions: a mobile shell cannot resolve which workspace is open, so it exports a device-local folder; a process that *can* resolve one aims at that workspace and nothing has to guess.

Three things that needed answering and are worth not re-deriving:

- **Why an inbox at all, when `outl daily append` already writes the block.** Not sandboxing — a terminal can open a workspace. It is that `append` boots the workspace (seconds on a large graph, for one line) and, when a GUI client already holds the device's write actor, falls back to an ephemeral one and leaves a fresh `ops-<ulid>.jsonl` per call. That is the litter root `CLAUDE.md` invariant 11 measures. `capture` pays neither.
- **The workspace inbox is inside the workspace, so the `file` transport can copy a capture to another device before the first one drained it.** Both then drain it and both emit `Create` for the same node id, because the block id is the capture's ULID. `do_op(Create)` is idempotent, so they converge on one block — the same property the crash replay above already leans on, reached by a second route.
- **The TUI drains at launch.** `outl capture` is a terminal command, so the terminal client is the one most likely to be open when it runs; not draining would make the command's promise false exactly there. It is a launch-time drain only, because nothing in the TUI watches the folder — recorded as `Partial` on the TUI row of `CaptureOutsideTheApp`.

Per-client coverage is `Capability::CaptureOutsideTheApp` in `outl_shortcuts::capability_support`, rendered into `docs/client-parity.md` (root `CLAUDE.md` invariant 12).

### What this amendment makes worse

The two inboxes are two places a capture can wait, and `drain_capture_inbox` drains both, so "where is my line" has two answers on Android (the only platform with both). `outl capture --list` reads only the workspace one.

A desktop capture also has no gesture of its own: outl registers no Service, menu bar item or global hotkey that writes one, so the user wires `outl capture` themselves. The row says so rather than claiming parity with a widget.

## Scope

Not covered here: capture to a page other than today, attachments in the share extension, and a macOS Service or Share Extension shipped by outl itself (today's desktop answer is `outl capture` plus whatever the user wires it to). Each needs its own issue.
