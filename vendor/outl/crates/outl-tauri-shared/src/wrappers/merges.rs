//! Merged-concurrent-edit commands (sync panel). Their own macro because
//! `catalog.rs` sits at its size ceiling; same shape as the rest.

/// The sync panel's merged-edits log and its undo.
#[macro_export]
macro_rules! merge_commands {
    ($state:ty) => {
        $crate::tauri_commands! {
            state = $state;

            /// Concurrent edits the CRDT merged this session, newest first.
            fn merged_edits()
                -> ::std::result::Result<::std::vec::Vec<$crate::merges::MergedEditDto>, String>
                => $crate::merges::merged_edits;

            /// Put this device's pre-merge text back (new ops) and drop the entry.
            fn undo_merged_edit(id: String)
                -> ::std::result::Result<$crate::state::PageView, String>
                => $crate::merges::undo_merged_edit;
        }
    };
}
