//! Emit an `outl batch` payload that creates N pages, each with a
//! small content forest.
//!
//! Output goes to stdout as a single `{"ops": [...]}` JSON object so
//! it can be piped straight into `outl batch`:
//!
//! ```text
//! cargo run -p xtask --bin gen-10k --release -- --pages 10000 \
//!     | outl -w /tmp/ws batch --json
//! ```
//!
//! Used by `bench-cli-xlarge` in `.github/workflows/bench.yml` to
//! exercise the CLI + op log + sqlite + sidecar + md-write pipeline
//! end-to-end at scale. Keep this generator deterministic — same
//! flags must produce the same payload across runs so a perf
//! regression is the signal, not fixture drift.

use clap::Parser;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
#[command(
    name = "gen-10k",
    about = "Generate an outl batch payload (N pages with M blocks each)."
)]
struct Args {
    /// Number of pages to create.
    #[arg(long, default_value_t = 10_000)]
    pages: usize,
    /// Top-level blocks per page (each gets one nested child).
    #[arg(long, default_value_t = 3)]
    blocks_per_page: usize,
    /// Fraction (1-in-N) of blocks tagged `#meeting` so `outl query
    /// --tag=meeting` has something to return.
    #[arg(long, default_value_t = 7)]
    tag_every: usize,
}

fn main() {
    let args = Args::parse();
    let mut ops: Vec<Value> = Vec::with_capacity(args.pages);

    for i in 0..args.pages {
        let slug = format!("page-{i:05}");
        let title = format!("Page {i:05}");
        let content: Vec<Value> = (0..args.blocks_per_page)
            .map(|j| build_block(i, j, args.tag_every))
            .collect();
        ops.push(json!({
            "op": "page_create",
            "args": {
                "slug": slug,
                "title": title,
                "content": content
            }
        }));
    }

    let payload = json!({ "ops": ops });
    // Streamed write avoids buffering the whole JSON in a String
    // before flushing — matters once `--pages` reaches the millions.
    serde_json::to_writer(std::io::stdout(), &payload).expect("write batch payload to stdout");
}

fn build_block(page_idx: usize, block_idx: usize, tag_every: usize) -> Value {
    let mut text = format!("block {block_idx} on page {page_idx:05}");
    if tag_every > 0 && (page_idx * 7 + block_idx).is_multiple_of(tag_every) {
        text.push_str(" #meeting");
    }
    json!({
        "text": text,
        "children": [
            { "text": format!("detail under block {block_idx}") }
        ]
    })
}
