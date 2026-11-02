//! Which journal days in a range have something written on them.
//!
//! Mobile's week strip puts a dot under each such day. "Has a journal
//! page" is the wrong question for that: opening a day creates its page,
//! so every day the user merely looked at would get a dot. A day counts
//! once one of its blocks holds text.

use std::collections::BTreeSet;

use outl_md::block_index::BlockEntry;
use outl_md::index::WorkspaceIndex;

use crate::host::AppHost;

/// Journal slugs between `from` and `to` (inclusive, `YYYY-MM-DD`) whose
/// page has at least one non-empty block, in ascending order.
///
/// Async: the index build reads and parses every `.md` + sidecar in the
/// workspace, and mobile's week strip asks for this on mount, beside the
/// journal it is also waiting to paint ([`crate::offload`]).
pub async fn journal_days_with_content<S: AppHost>(
    state: &S,
    from: String,
    to: String,
) -> Result<Vec<String>, String> {
    let root = state.storage_root()?;
    crate::offload::run("journal_days_with_content", move || {
        let index = WorkspaceIndex::build(&root);
        let journals: BTreeSet<&str> = index
            .pages()
            .filter(|p| p.is_journal)
            .map(|p| p.slug.as_str())
            .collect();
        Ok(days_with_content(
            index
                .iter_blocks()
                .filter(|b| journals.contains(b.source_slug.as_str())),
            &from,
            &to,
        ))
    })
    .await
}

/// The range filter behind [`journal_days_with_content`], over journal
/// blocks only. ISO slugs sort as dates, so the range is a string range.
pub fn days_with_content<'a>(
    journal_blocks: impl Iterator<Item = &'a BlockEntry>,
    from: &str,
    to: &str,
) -> Vec<String> {
    journal_blocks
        .filter(|b| b.source_slug.as_str() >= from && b.source_slug.as_str() <= to)
        .filter(|b| !b.text.trim().is_empty())
        .map(|b| b.source_slug.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use outl_core::id::NodeId;

    fn block(slug: &str, text: &str) -> BlockEntry {
        BlockEntry {
            id: NodeId::new(),
            ref_handle: "blk-000000".into(),
            source_slug: slug.into(),
            source_path: format!("{slug}.md").into(),
            source_block_path: vec![0],
            text: text.into(),
            text_fold: text.to_lowercase(),
            properties: Vec::new(),
            children: Vec::new(),
        }
    }

    #[test]
    fn a_day_counts_once_a_block_holds_text() {
        let blocks = [
            block("2026-10-05", "before the range"),
            block("2026-10-06", "first day"),
            block("2026-10-06", "same day twice"),
            block("2026-10-08", "   "),
            block("2026-10-12", "last day"),
            block("2026-10-13", "after the range"),
        ];
        let days = days_with_content(blocks.iter(), "2026-10-06", "2026-10-12");
        assert_eq!(days, ["2026-10-06", "2026-10-12"]);
    }
}
