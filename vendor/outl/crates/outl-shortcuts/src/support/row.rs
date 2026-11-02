//! The `row!` shorthand for the support table, split out of
//! `support.rs` because that file sits at its size ceiling.

/// Shorthand so the table below reads as three columns.
macro_rules! row {
    ($tui:expr, $desktop:expr, $mobile:expr) => {
        ClientSupport {
            tui: $tui,
            desktop: $desktop,
            mobile: $mobile,
        }
    };
}
