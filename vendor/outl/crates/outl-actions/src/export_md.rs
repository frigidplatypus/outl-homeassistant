//! Copy the workspace out as plain Markdown, one click from any client.
//!
//! The `.md` files under `journals/` and `pages/` already are the user's
//! notes, clean (root `CLAUDE.md` invariant 2). Export is therefore a
//! copy, not a render: what lands in the folder is byte for byte what
//! outl keeps, plus the `assets/` the pages link to. Sidecars (`.outl`)
//! and the op log stay behind, because they are outl's bookkeeping and
//! not something a person opening the folder in any other editor needs.
//!
//! The destination is a fresh `outl-export-<stamp>` folder inside the
//! directory the client picks, so an export never overwrites anything.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::journal::{journals_dir, pages_dir};

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    /// The folder that was created.
    pub dir: PathBuf,
    /// How many `.md` files were copied.
    pub pages: usize,
    /// How many asset files were copied.
    pub assets: usize,
}

/// Copy every `.md` page and every asset from `root` into a new
/// `outl-export-<stamp>` folder under `dest_parent`.
///
/// `stamp` comes from the caller so tests (and two exports in the same
/// second) stay deterministic; a taken name gets a `-2`, `-3` suffix.
pub fn export_markdown(root: &Path, dest_parent: &Path, stamp: &str) -> io::Result<ExportReport> {
    fs::create_dir_all(dest_parent)?;
    let dir = free_dir(dest_parent, stamp);
    fs::create_dir(&dir)?;
    let mut pages = 0;
    for (src, name) in [(journals_dir(root), "journals"), (pages_dir(root), "pages")] {
        pages += copy_tree(&src, &dir.join(name), &|p| {
            p.extension().is_some_and(|e| e == "md")
        })?;
    }
    let assets = copy_tree(&root.join("assets"), &dir.join("assets"), &|_| true)?;
    Ok(ExportReport { dir, pages, assets })
}

fn free_dir(parent: &Path, stamp: &str) -> PathBuf {
    let base = parent.join(format!("outl-export-{stamp}"));
    if !base.exists() {
        return base;
    }
    (2..)
        .map(|n| parent.join(format!("outl-export-{stamp}-{n}")))
        .find(|p| !p.exists())
        .expect("an unbounded range always yields a free name")
}

/// Recursive copy of the files `keep` accepts. A missing source is an
/// empty workspace section, not an error. Dotfiles are skipped.
fn copy_tree(src: &Path, dst: &Path, keep: &dyn Fn(&Path) -> bool) -> io::Result<usize> {
    let entries = match fs::read_dir(src) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut copied = 0;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copied += copy_tree(&path, &dst.join(entry.file_name()), keep)?;
        } else if kind.is_file() && keep(&path) {
            fs::create_dir_all(dst)?;
            fs::copy(&path, dst.join(entry.file_name()))?;
            copied += 1;
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests;
