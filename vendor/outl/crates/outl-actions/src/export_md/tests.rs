//! Export copies the user's notes out, and only the notes.

use std::fs;

use super::*;

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

#[test]
fn copies_pages_journals_and_assets_but_not_bookkeeping() {
    let ws = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(&root.join("pages/foo.md"), "- hello\n");
    write(&root.join("pages/foo.outl"), "{}");
    write(&root.join("pages/ns/bar.md"), "- nested\n");
    write(&root.join("journals/2026_10_12.md"), "- today\n");
    write(&root.join("assets/pic.png"), "png");
    write(&root.join("ops/ops-a.jsonl"), "{}");
    write(&root.join("pages/.hidden.md"), "x");

    let report = export_markdown(root, out.path(), "2026-10-12").unwrap();

    assert_eq!(report.pages, 3);
    assert_eq!(report.assets, 1);
    let dir = out.path().join("outl-export-2026-10-12");
    assert_eq!(report.dir, dir);
    assert_eq!(
        fs::read_to_string(dir.join("pages/foo.md")).unwrap(),
        "- hello\n"
    );
    assert!(dir.join("pages/ns/bar.md").exists());
    assert!(dir.join("journals/2026_10_12.md").exists());
    assert!(dir.join("assets/pic.png").exists());
    assert!(!dir.join("pages/foo.outl").exists());
    assert!(!dir.join("ops").exists());
    assert!(!dir.join("pages/.hidden.md").exists());
}

/// A second export the same minute must not land on top of the first.
#[test]
fn never_overwrites_a_previous_export() {
    let ws = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    write(&ws.path().join("pages/a.md"), "- a\n");
    let first = export_markdown(ws.path(), out.path(), "s").unwrap();
    let second = export_markdown(ws.path(), out.path(), "s").unwrap();
    assert_ne!(first.dir, second.dir);
    assert!(second.dir.ends_with("outl-export-s-2"));
}

#[test]
fn an_empty_workspace_exports_an_empty_folder() {
    let ws = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let report = export_markdown(ws.path(), out.path(), "e").unwrap();
    assert_eq!((report.pages, report.assets), (0, 0));
    assert!(report.dir.is_dir());
}
