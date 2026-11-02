use std::fs;
use std::path::Path;

use chrono::NaiveDate;
use outl_core::hlc::HlcGenerator;
use outl_core::id::{ActorId, NodeId};
use outl_core::workspace::Workspace;
use ulid::Ulid;

use super::*;
use crate::dates::journal_slug;
use crate::page::find_by_slug;
use crate::tree::children_of;

fn ws() -> (Workspace, HlcGenerator) {
    let actor = ActorId::new();
    (
        Workspace::open_in_memory(actor).unwrap(),
        HlcGenerator::new(actor),
    )
}

const DAY_MS: i64 = 86_400_000;
const T0: i64 = 1_760_000_000_000;

fn day_of(ms: i64) -> NaiveDate {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap()
        .date_naive()
}

/// Put one capture in the inbox through the real writer, so these tests
/// read what an extension actually leaves behind rather than a second
/// hand-written spelling of the same format.
fn stage_capture(dir: &Path, ms: i64, text: &str) -> Ulid {
    let path = write_capture(dir, text, "intent", ms).unwrap();
    capture_id(&path).expect("the writer publishes a <ULID>.json name")
}

fn texts(w: &Workspace, page: NodeId) -> Vec<String> {
    children_of(w, page)
        .into_iter()
        .map(|(id, _)| w.block_text(id).unwrap_or_default())
        .collect()
}

#[test]
fn drain_appends_in_capture_order() {
    let dir = tempfile::tempdir().unwrap();
    // Written newest first, so mtime order is the reverse of capture order.
    stage_capture(dir.path(), T0 + 3_000, "third");
    stage_capture(dir.path(), T0 + 1_000, "first");
    stage_capture(dir.path(), T0 + 2_000, "second");
    let (mut w, hlc) = ws();

    let report = drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    let page = find_by_slug(&w, &journal_slug(day_of(T0))).unwrap();
    assert_eq!(texts(&w, page), ["first", "second", "third"]);
    assert_eq!(report.landed.len(), 3);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "landed files are deleted"
    );
}

#[test]
fn drain_is_idempotent_after_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    stage_capture(dir.path(), T0, "call the venue");
    let (mut w, hlc) = ws();
    // The op committed, then the process died before the delete.
    let (pending, _) = pending_captures(dir.path());
    let page = capture_page(&mut w, &hlc, day_of(T0)).unwrap();
    land_capture(&mut w, &hlc, page, &pending[0]).unwrap();

    drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    assert_eq!(texts(&w, page), ["call the venue"]);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn capture_lands_in_the_day_it_was_made() {
    let dir = tempfile::tempdir().unwrap();
    stage_capture(dir.path(), T0 - DAY_MS, "from yesterday");
    let (mut w, hlc) = ws();

    drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    let yesterday = find_by_slug(&w, &journal_slug(day_of(T0 - DAY_MS))).unwrap();
    assert_eq!(texts(&w, yesterday), ["from yesterday"]);
    assert!(find_by_slug(&w, &journal_slug(day_of(T0))).is_none());
}

#[test]
fn partial_file_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    // The name `outl_md::atomic`'s `tmp_path` actually publishes for
    // `<ULID>.json`: the filename with a `.tmp` suffix *and* a leading
    // dot. Spelling it any other way makes this test pass for a string
    // the writer never emits, which is what it did when the hand-rolled
    // writer here was swapped for `write_atomic`.
    let tmp = dir
        .path()
        .join(format!(".{}.json.tmp", Ulid::from_parts(T0 as u64, 99)));
    fs::write(&tmp, "{\"text\":\"half").unwrap();
    let (mut w, hlc) = ws();

    let report = drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    assert!(report.landed.is_empty());
    assert!(tmp.exists(), "a temp file is left for the next drain");
}

#[test]
fn a_malformed_capture_is_kept_and_counted() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir
        .path()
        .join(format!("{}.json", Ulid::from_parts(T0 as u64, 99)));
    fs::write(&bad, "not json").unwrap();
    stage_capture(dir.path(), T0, "fine");
    let (mut w, hlc) = ws();

    let report = drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    assert_eq!(report.skipped, 1);
    assert_eq!(report.landed.len(), 1);
    assert!(bad.exists());
}

#[test]
fn the_block_id_is_the_capture_ulid() {
    let dir = tempfile::tempdir().unwrap();
    let id = stage_capture(dir.path(), T0, "keep my time");
    let (mut w, hlc) = ws();

    let report = drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    // Asked of the tree, not of the report: the report *derives* the id
    // from the capture, so reading it back from there would assert that
    // one line against itself. What has to be true is that the block in
    // the workspace is that id — it is what makes a replay after a crash
    // land on the same block instead of a second one.
    assert_eq!(report.landed[0].1, NodeId(id));
    assert!(w.tree().contains(NodeId(id)), "the block is the capture id");
    assert_eq!(w.block_text(NodeId(id)).as_deref(), Some("keep my time"));
}

#[test]
fn a_written_capture_reads_back_whole() {
    let dir = tempfile::tempdir().unwrap();

    write_capture(dir.path(), "  ring the dentist  ", "share", T0).unwrap();

    let (pending, skipped) = pending_captures(dir.path());
    assert_eq!(skipped, 0);
    assert_eq!(
        pending[0].record,
        CaptureRecord {
            text: "ring the dentist".to_string(),
            captured_at_ms: T0,
            source: "share".to_string(),
        },
        "the writer trims and the reader sees every field"
    );
}

#[test]
fn written_captures_sort_in_capture_order() {
    let dir = tempfile::tempdir().unwrap();
    // Written newest first: only the stamp in the name may decide.
    write_capture(dir.path(), "third", "share", T0 + 3_000).unwrap();
    write_capture(dir.path(), "first", "share", T0 + 1_000).unwrap();
    write_capture(dir.path(), "second", "share", T0 + 2_000).unwrap();

    let (pending, _) = pending_captures(dir.path());

    let texts: Vec<_> = pending.iter().map(|c| c.record.text.as_str()).collect();
    assert_eq!(texts, ["first", "second", "third"]);
}

#[test]
fn a_blank_capture_is_refused_at_the_writer() {
    let dir = tempfile::tempdir().unwrap();

    let refused = write_capture(dir.path(), "   \n\t ", "share", T0);

    assert!(matches!(refused, Err(ActionError::EmptyCapture)));
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "nothing is published, not even a temp file"
    );
}

#[test]
fn the_writer_leaves_only_the_published_name_behind() {
    let dir = tempfile::tempdir().unwrap();

    let path = write_capture(dir.path(), "one line", "share", T0).unwrap();

    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [path.file_name().unwrap().to_string_lossy().into_owned()],
        "the temp file is renamed, never left next to the capture"
    );
}

#[test]
fn the_writer_creates_an_inbox_that_is_not_there_yet() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("captures");

    write_capture(&inbox, "first ever capture", "share", T0).unwrap();

    let (pending, _) = pending_captures(&inbox);
    assert_eq!(pending.len(), 1);
}

#[test]
fn a_capture_written_before_1970_still_sorts_first() {
    let dir = tempfile::tempdir().unwrap();
    // A device clock behind the epoch: the stamp must not wrap into a
    // far-future ULID that sorts after every real capture.
    write_capture(dir.path(), "clock was wrong", "share", -5_000).unwrap();
    write_capture(dir.path(), "clock was right", "share", T0).unwrap();

    let (pending, _) = pending_captures(dir.path());

    let texts: Vec<_> = pending.iter().map(|c| c.record.text.as_str()).collect();
    assert_eq!(texts, ["clock was wrong", "clock was right"]);
}

/// The cases every `shared_line` implementation answers live in
/// `tests/shared-line-cases.json`, read by `shared_line_parity.rs` here
/// and by `SharedLineParityTests.swift` there. This file deliberately
/// holds no copy of them: a third spelling of the rule is the drift that
/// fixture exists to remove.
///
/// What belongs here is the half the fixture *cannot* carry.
#[test]
fn a_multi_line_url_is_still_one_line() {
    // Rust-only on purpose, and the reason is why this bug survived the
    // parity fixture: the Swift twin takes a typed `URL?`, and the Swift
    // test builds its argument with `URL.init(string:)`, which returns
    // nil for a string holding a newline. The case is therefore
    // inexpressible on that side and cannot go in the shared file.
    //
    // Rust reaches it through Android: with a subject present,
    // `OutlShareActivity` pairs the extras as `(subject, text)`, so a
    // multi-line selection shared from a browser arrives in the `url`
    // slot, which used to be `.trim()`-only.
    assert_eq!(
        shared_line(Some("Title"), Some("first\nsecond")).as_deref(),
        Some("Title first second"),
        "the url half collapses newlines like the text half"
    );
    assert_eq!(
        shared_line(None, Some("  first\n\n  second  ")).as_deref(),
        Some("first second"),
        "a url-only share is normalized too"
    );
}

/// Write a capture the way a writer *outside* this crate does: the
/// record straight to `<ULID>.json`, with no normalization applied.
///
/// That is what an iOS extension, a Shortcut handing the App Intent a
/// multi-line string, or a peer's capture file arriving over a `file`
/// transport actually looks like. Staging through `write_capture` would
/// test the reader against the one writer that already normalizes, which
/// is the gap these tests exist to close.
fn stage_raw_capture(dir: &Path, ms: i64, text: &str) -> Ulid {
    let id = Ulid::from_datetime(capture_instant(ms));
    let record = CaptureRecord {
        text: text.to_string(),
        captured_at_ms: ms,
        source: "share".to_string(),
    };
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    id
}

#[test]
fn a_capture_from_an_outside_writer_lands_as_one_line() {
    // Landing this raw would project as indented continuation lines,
    // which the parser reads back as tree structure and `key:: value`
    // properties — several blocks plus a forged `title::`, with the
    // capture's own block trashed as an orphan on the next reconcile.
    let dir = tempfile::tempdir().unwrap();
    stage_raw_capture(
        dir.path(),
        T0,
        "innocent\n- forged sibling\ntitle:: Hijacked",
    );
    let (mut w, hlc) = ws();

    drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    let page = find_by_slug(&w, &journal_slug(day_of(T0))).unwrap();
    assert_eq!(
        texts(&w, page),
        ["innocent - forged sibling title:: Hijacked"],
        "one capture is one block, however it was written"
    );
}

#[test]
fn a_torn_landing_is_completed_by_the_next_drain() {
    // Landing is two independently fsynced ops. This is the state a
    // kill between them leaves: `Op::Create` replayed, `Op::Edit` never
    // written, so the block is present and empty. The guard used to read
    // "present" as "landed", return Ok, and let the drain delete the
    // file — losing the line for good.
    let dir = tempfile::tempdir().unwrap();
    stage_capture(dir.path(), T0, "call the venue");
    let (mut w, hlc) = ws();
    let (pending, _) = pending_captures(dir.path());
    let page = capture_page(&mut w, &hlc, day_of(T0)).unwrap();
    let position = position_for_new_last_child(&w, page);
    crate::block::create_with_explicit_id(
        &mut w,
        &hlc,
        NodeId(pending[0].id),
        page,
        position,
        None,
    )
    .unwrap();
    assert_eq!(texts(&w, page), [""], "the torn state: present but empty");

    drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    assert_eq!(texts(&w, page), ["call the venue"]);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "the file is only deleted once the text is really there"
    );
}

#[test]
fn a_landed_capture_that_was_edited_is_left_alone() {
    // The other half of the fix above, and the reason it tests for
    // *empty* rather than *different*: this capture file reaches a
    // second device through a `file` transport, so a second drain sees a
    // block the user has since edited. Re-asserting on divergence would
    // overwrite that edit.
    let dir = tempfile::tempdir().unwrap();
    stage_capture(dir.path(), T0, "call the venue");
    let (mut w, hlc) = ws();
    let (pending, _) = pending_captures(dir.path());
    let page = capture_page(&mut w, &hlc, day_of(T0)).unwrap();
    land_capture(&mut w, &hlc, page, &pending[0]).unwrap();
    crate::block::edit_text(&mut w, &hlc, NodeId(pending[0].id), "call the venue at 3pm").unwrap();

    drain_capture_dir(&mut w, &hlc, dir.path(), day_of).unwrap();

    assert_eq!(texts(&w, page), ["call the venue at 3pm"]);
}

#[test]
fn an_oversized_capture_is_counted_not_read() {
    // The drain runs on every return to the foreground and leaves a file
    // it cannot land, so an unbounded read is paid again on every pass
    // for the life of the install. Counted, not deleted: `skipped`
    // reaches the user now, and silently dropping someone's capture
    // would be the worse answer.
    let dir = tempfile::tempdir().unwrap();
    let id = Ulid::from_datetime(capture_instant(T0));
    let fat = "x".repeat((MAX_CAPTURE_BYTES as usize) + 1);
    fs::write(
        dir.path().join(format!("{id}.json")),
        serde_json::to_vec(&CaptureRecord {
            text: fat,
            captured_at_ms: T0,
            source: "share".to_string(),
        })
        .unwrap(),
    )
    .unwrap();

    let (pending, skipped) = pending_captures(dir.path());

    assert!(pending.is_empty(), "an oversized file is not a capture");
    assert_eq!(skipped, 1, "and the user is told it is still sitting there");
}

#[test]
fn the_file_uses_the_keys_the_swift_writer_expects() {
    // `OutlKit`'s `CaptureInboxTests` pins the same three keys from the
    // other side. Without this half, a field added here (non-`Option`,
    // so required on the way back in) would keep passing every Rust test
    // while every Swift-written capture became an unparseable file that
    // `pending_captures` counts in `skipped` and leaves in the inbox
    // forever — a `tracing::warn!` the user never sees.
    let dir = tempfile::tempdir().unwrap();
    let path = write_capture(dir.path(), "read this", "intent", T0).unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).expect("the record is JSON");

    let mut keys: Vec<_> = raw
        .as_object()
        .expect("the record is a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["captured_at_ms", "source", "text"],
        "the format has a second implementation in Swift; changing this \
         key set needs `OutlKit/Capture/CaptureInbox.swift` changed with it"
    );
}
