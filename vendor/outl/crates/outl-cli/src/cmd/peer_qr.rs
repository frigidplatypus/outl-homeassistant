//! Pairing tickets ↔ terminal QR codes.
//!
//! A ticket runs 400 to 750 characters, which nobody is going to retype, and the
//! mobile app pairs by pointing its camera at a QR
//! (`tauri-plugin-barcode-scanner`). So the QR is not decoration — for a phone
//! it is the only practical way in.
//!
//! Which makes width the thing that matters. Rendered with the `Dense1x2`
//! block renderer a ticket QR is 77 to 101 columns, and a terminal narrower
//! than that wraps every row. **A wrapped QR is not a degraded QR, it is noise** —
//! the phone will never decode it, and nothing on screen says why. That is the
//! whole reason this module exists rather than a `println!` at the call site:
//! somebody has to measure before printing.
//!
//! The two callers want opposite defaults, and both are right:
//!
//! - `outl peer pair` prints a QR *incidentally*, on the way to a ticket it
//!   also prints as text. If the QR cannot fit, printing it anyway buries the
//!   ticket under fifty lines of garbage — so it is skipped, with one line
//!   saying what would have made it fit.
//! - `outl peer qr` was asked for the QR and nothing else. Refusing to print
//!   it there would leave the user with no way to get one at all, so it prints
//!   and warns.

use std::io::{IsTerminal, Read};

use anyhow::{bail, Context, Result};

/// Resolve the ticket a command was given.
///
/// `None` or `"-"` reads stdin to end-of-file, which is what makes a ticket of
/// several hundred characters bearable to move into a container:
///
/// ```text
/// pbpaste | docker compose run -i --rm outl peer pair --ticket -
/// ```
///
/// Refuses to read an interactive stdin: a bare `outl peer qr` on a terminal
/// would otherwise sit there looking hung with no prompt.
pub fn read_ticket(arg: Option<&str>) -> Result<String> {
    if let Some(t) = arg.filter(|t| *t != "-") {
        return Ok(t.trim().to_string());
    }
    if std::io::stdin().is_terminal() {
        bail!(
            "no ticket given. Pass it as an argument, or pipe it in:\n  \
             pbpaste | outl peer qr -"
        );
    }
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("read ticket from stdin")?;
    let ticket = buf.trim().to_string();
    if ticket.is_empty() {
        bail!("stdin was empty — expected a pairing ticket");
    }
    Ok(ticket)
}

/// How many terminal columns a rendered QR needs.
///
/// The `Dense1x2` renderer packs two module rows into one character row, so
/// height is halved but **width is one character per module** — and width is
/// what a terminal runs out of. Counted in `char`s, not bytes: every glyph the
/// renderer emits (`█`, `▀`, `▄`, space) is one column, and three of those four
/// are multi-byte.
///
/// Returns 0 for an empty render, which no caller should read as "it fits".
fn qr_columns(rendered: &str) -> usize {
    rendered
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

/// Terminal width in columns, or `None` when there is no terminal.
///
/// `None` is "no constraint", not "assume 80": output redirected to a file or
/// a pipe has no width to overflow, and guessing one there would drop the QR
/// from exactly the capture the user redirected it into.
fn terminal_columns() -> Option<usize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    crossterm::terminal::size()
        .ok()
        .map(|(cols, _)| cols as usize)
}

/// Print `ticket` as a QR when it fits, or one line explaining why it did not.
///
/// For the `outl peer pair` path, where the ticket is printed as text too and
/// a wrapped QR would bury it.
pub fn print_ticket_qr_if_it_fits(ticket: &str) -> Result<()> {
    let qr = outl_sync_iroh::ticket_qr(ticket).context("render the pairing QR")?;
    let needed = qr_columns(&qr);
    match terminal_columns() {
        Some(have) if have < needed => {
            println!(
                "(The pairing QR needs {needed} columns and this terminal has {have}, \
                 so it is not shown."
            );
            println!(
                " Widen the window and re-run, or pipe the ticket below into \
                 `outl peer qr -`.)"
            );
        }
        _ => {
            println!("Scan this QR on the other device, or copy the ticket:");
            println!();
            println!("{qr}");
        }
    }
    Ok(())
}

/// Run `outl peer qr` — render a ticket the caller already has.
///
/// Always prints. The width warning goes to stderr so piping the QR into a
/// file or a wider terminal keeps the QR clean.
pub fn run(ticket: Option<&str>) -> Result<()> {
    let ticket = read_ticket(ticket)?;
    let qr = outl_sync_iroh::ticket_qr(&ticket).context("render the pairing QR")?;
    let needed = qr_columns(&qr);
    if let Some(have) = terminal_columns() {
        if have < needed {
            eprintln!(
                "warning: this QR needs {needed} columns and this terminal has {have}; \
                 it will wrap and no camera will read it. Widen the window, or pipe \
                 this command's output somewhere wider."
            );
        }
    }
    println!("{qr}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_ticket_is_trimmed_not_read_from_stdin() {
        assert_eq!(
            read_ticket(Some("  outlpair1.abc  ")).unwrap(),
            "outlpair1.abc"
        );
    }

    /// The guard exists because a QR one column too wide is unreadable, not
    /// slightly worse. If this ever measures bytes instead of chars it will
    /// over-report by ~3x — every glyph but the space is multi-byte — and the
    /// QR gets suppressed on terminals that could have shown it.
    #[test]
    fn qr_width_is_counted_in_columns_not_bytes() {
        let qr = outl_sync_iroh::ticket_qr("outlpair1.short-ticket").unwrap();
        let cols = qr_columns(&qr);
        let widest_line_bytes = qr.lines().map(str::len).max().unwrap();
        assert!(cols > 0, "a rendered QR has width");
        assert!(
            widest_line_bytes > cols,
            "block glyphs are multi-byte, so bytes must exceed columns \
             (bytes {widest_line_bytes}, columns {cols})"
        );
    }
}
