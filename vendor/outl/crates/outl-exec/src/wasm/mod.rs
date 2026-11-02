//! WebAssembly sandbox — the **uniform** execution path that every
//! language can eventually share.
//!
//! Today only the [`crate::runtimes::rust`] runtime uses it (because
//! Rust has no in-process interpreter — we compile to `wasm32-wasip1`
//! and run via wasmtime). Tomorrow the in-process interpreters
//! (Steel, Boa, RustPython, mlua) will be swapped for WASM-built
//! equivalents and route through the same code path.
//!
//! Gated behind the `wasm` Cargo feature.
//!
//! ## Pieces
//!
//! - [`engine`] — builds a `wasmtime::Engine` configured with the
//!   uniform sandbox: WASI with no preopens / no env / no sockets,
//!   wall-clock timeout via epoch interruption, optional instruction
//!   limit via fuel.
//! - [`module`] — [`WasmModule`], the adapter that takes a WASI module
//!   binary plus a source string, runs it through wasmtime, and
//!   produces an [`crate::ExecOutput`]. Implements
//!   [`crate::Runtime`].
//! - [`cache`] — `~/.cache/outl/runtimes/` on Linux,
//!   `~/Library/Caches/outl/runtimes/` on macOS, etc.
//!   Lazy-generated `.wasm` blobs (today: Rust source → WASM)
//!   land here keyed by SHA-256 of the source.

pub mod cache;
pub mod engine;
pub mod module;
// `sha2` is now an unconditional dep (used by auto-run hashing too),
// but cache.rs only needs it under `wasm`; nothing to change here.

pub use cache::{cache_dir, cache_path_for_source};
pub use engine::{make_engine, SandboxLimits};
pub use module::WasmModule;
