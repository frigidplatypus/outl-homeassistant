//! Who owns which process-wide global on Android.
//!
//! Two JVM-backed globals have to be primed before iroh's first QUIC
//! connection, and they have **different owners**:
//!
//! | global | owner | primed in |
//! |---|---|---|
//! | `rustls-platform-verifier`'s JNI handle | this crate | `android_jni.rs`, from `MainActivity.onCreate` |
//! | `ndk_context` | `tao` | `ndk_glue::onCreate`, inside `Rust_onCreate` |
//!
//! Getting the second row wrong costs the whole app, and costs it
//! silently. `ndk_context::initialize_android_context` is documented
//! as "must be called **exactly once**" and enforces that with
//! `assert!(previous.is_none())`. `android_jni.rs` used to call it
//! from `MainActivity.onCreate`, which runs *before* `super.onCreate`
//! reaches tao — so tao's own call found the slot full, the assert
//! panicked, the panic unwound into an `extern "C"` frame, and
//! `panic_cannot_unwind` aborted the process. Every Android device,
//! every clean install, with nothing in the build going red.
//!
//! The test below is a **manifest** check rather than a code check on
//! purpose. `android_jni.rs` is `#[cfg(target_os = "android")]`, so a
//! `#[cfg(test)]` module inside it never runs in CI (the suite runs on
//! the host), which is exactly how this shipped in the first place.
//! The manifest compiles into every target, host included.
//!
//! It also catches the honest version of the mistake. Nobody is going
//! to retype `initialize_android_context` against the comment sitting
//! above it; what happens is that somebody needs a Context in a new
//! Android file, adds `ndk-context` back as a direct dependency
//! because that is what you do, and reaches for the same function.
//! Taking the name out of reach is what makes them read the comment.
//!
//! `ndk-context` stays in the dependency graph transitively, for tao
//! to write and `iroh-dns` to read. This is only about *naming* it
//! here.
//!
//! **What this does not catch:** a second call made through some other
//! path — a transitive re-export, or another crate we add that primes
//! it too. That is the same class of gap `capability_parity` declares
//! about `invoke_handler!`, and the thing that would actually close it
//! is a launch smoke test on a real emulator, which does not exist yet
//! ([`docs/android-platform.md`](../../../../docs/android-platform.md)).
//!
//! Test-only — there is nothing here for the running app to call.

#[cfg(test)]
mod tests {
    /// Fails if `ndk-context` comes back as a **direct** dependency of
    /// this crate.
    ///
    /// Parsed rather than grepped, for the reason
    /// `outl_telemetry::tests::the_sentry_feature_is_off_by_default`
    /// gives: a textual check passes against the same dependency
    /// written with different spacing, or under
    /// `dependencies.ndk-context`, and this test is what stands
    /// between a plausible edit and an app that will not open.
    #[test]
    fn ndk_context_is_not_a_direct_dependency() {
        // `Table`, not `Value`: in toml 1.x `Value`'s `FromStr` parses a
        // single value, and a whole document is a `Table`.
        let manifest: toml::Table = include_str!("../Cargo.toml")
            .parse()
            .expect("Cargo.toml must parse as TOML");

        let mut tables = vec![manifest.get("dependencies")];
        // Every `[target.'cfg(...)'.dependencies]`, whatever the cfg
        // spells — a new target table must not be a way back in.
        if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
            for (_cfg, table) in targets {
                tables.push(table.get("dependencies"));
                tables.push(table.get("dev-dependencies"));
            }
        }

        for table in tables.into_iter().flatten() {
            assert!(
                table.get("ndk-context").is_none(),
                "outl-mobile must not depend on `ndk-context` directly: `tao` owns that \
                 global and seeds it inside `Rust_onCreate`. A second \
                 `initialize_android_context` is an `assert!` failure that aborts the app \
                 on launch, on every device. See `android_jni.rs` and \
                 `docs/android-platform.md`."
            );
        }
    }
}
