//! Thin orchestration over `outl_md::reconcile`.
//!
//! The reconcile primitive itself lives in `outl-md` so that both the
//! CLI (file watcher) and the TUI (post-edit commits) can call it.

use crate::workspace_layout::Paths;
use anyhow::Result;
use outl_core::hlc::HlcGenerator;
use outl_core::workspace::Workspace;
pub use outl_md::reconcile::ReconcileReport;
use outl_md::reconcile::{reconcile_dir as md_reconcile_dir, reconcile_md as md_reconcile_md};
use std::path::Path;

/// Reconcile a single `.md` file, logging orphans to `<.outl>/orphans.log`.
pub fn reconcile_md(
    ws: &mut Workspace,
    hlc: &HlcGenerator,
    paths: &Paths,
    md_path: &Path,
) -> Result<ReconcileReport> {
    Ok(md_reconcile_md(ws, hlc, md_path, Some(&paths.orphans))?)
}

/// Reconcile every `.md` in a directory.
pub fn reconcile_dir(
    ws: &mut Workspace,
    hlc: &HlcGenerator,
    paths: &Paths,
    dir: &Path,
) -> Result<Vec<ReconcileReport>> {
    Ok(md_reconcile_dir(ws, hlc, dir, Some(&paths.orphans))?)
}
