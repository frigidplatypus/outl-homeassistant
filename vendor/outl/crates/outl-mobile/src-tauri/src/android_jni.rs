//! Android JNI bootstrap for iroh's TLS.
//!
//! iroh's relay client verifies certificates through `rustls-platform-verifier`,
//! which calls into the JVM (the Android KeyStore) and keeps its own `OnceCell`
//! that starts out empty. Neither Tauri/wry nor `tao` primes it, so on the first
//! QUIC connection the lookup panics; the panic then poisons a `quinn` (`noq`)
//! mutex, and the next task's `.unwrap()` on the `PoisonError` aborts the
//! process — surfacing as a `SIGABRT` on a `tokio-rt-worker` thread moments
//! after boot.
//!
//! `MainActivity.onCreate` calls [`Java_app_outl_mobile_1app_NativeSetup_install`]
//! **before** `super.onCreate` boots Tauri (and therefore before iroh makes its
//! first connection), handing us the `Application` context so we can prime that
//! global once, up front.
//!
//! # `ndk_context` is **not** ours to install
//!
//! The system-DNS reader (`iroh-dns` → `hickory` → `LinkProperties.getDnsServers()`)
//! reads the JavaVM + Context out of `ndk_context`, and this module used to seed
//! that too. It must not, and the cost of getting this wrong is the whole app:
//!
//! `ndk_context::initialize_android_context` is documented as "must be called
//! **exactly once**" and enforces it with `assert!(previous.is_none())`.
//! `tao`'s `ndk_glue::onCreate` already calls it, inside the
//! `APPLICATION_CONTEXT.get_or_init` that `Java_app_outl_mobile_1app_Rust_onCreate`
//! runs — with the same `getApplicationContext()` we were passing. That `OnceCell`
//! is tao's, so it cannot see a flag of ours, and seeding first did not stop the
//! second call. It made it fail.
//!
//! Seeding here therefore turned every launch into: we fill the slot in
//! `MainActivity.onCreate`, `super.onCreate` reaches tao, the assert trips, the
//! panic unwinds into an `extern "C"` frame, and `panic_cannot_unwind` aborts.
//! The app closed on open on **every** Android device, on a clean install, from
//! the day this file landed — unrelated to the 16 KB page-size bug that was
//! hiding in front of it.
//!
//! So the split is: **the verifier is ours, `ndk_context` is tao's.** tao seeds
//! it from inside `super.onCreate`, long before the transport opens a
//! connection, so the DNS reader still finds it. If tao ever stops, the symptom
//! is a QUIC failure at runtime rather than this module reclaiming the slot.

use std::sync::atomic::{AtomicBool, Ordering};

use jni::objects::{JClass, JObject};
use jni::{Env, EnvUnowned};

/// Set once the verifier is primed. `MainActivity.onCreate` can fire again
/// (process/activity recreation after a background kill), and this keeps the
/// repeat a cheap no-op. Flipped only after a successful install, so a failed
/// first attempt can still retry.
///
/// `rustls-platform-verifier` self-guards with its own `OnceCell`, so this flag
/// is belt-and-braces rather than the thing standing between us and a crash.
/// The global that did not tolerate a second call, `ndk_context`, is no longer
/// written here at all — see the module docs.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// JNI entry point invoked from `MainActivity` (`NativeSetup.install`).
///
/// Primes `rustls-platform-verifier`'s global JNI handle so iroh's TLS
/// verification works on Android. `with_env` wraps the body in `catch_unwind`,
/// and [`jni::errors::LogErrorAndDefault`] logs (never throws) on failure — a
/// thrown exception here would crash `onCreate`. If priming fails the app still
/// boots; iroh may then misbehave, but that is recoverable where an aborted
/// process is not.
///
/// # Safety
///
/// Called by the JVM with a valid JNI env, class, and non-null `Context`.
#[no_mangle]
pub extern "system" fn Java_app_outl_mobile_1app_NativeSetup_install<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) {
    unowned_env
        .with_env(|env: &mut Env| install(env, context))
        .resolve::<jni::errors::LogErrorAndDefault>();
}

/// Prime the one JVM-backed global that is ours. Runs once at boot.
///
/// Deliberately short. Everything this function used to do besides the
/// verifier belonged to `tao`, and doing it here first is what aborted the
/// process — see the module docs before adding anything back.
fn install(env: &mut Env, context: JObject) -> Result<(), jni::errors::Error> {
    // Single-threaded (UI thread), so a plain load/store is enough.
    if INSTALLED.load(Ordering::Acquire) {
        return Ok(());
    }

    // TLS cert verification (iroh relay client → rustls). The verifier reads
    // the Android system trust store via JNI and `.expect(...)`s its `OnceCell`
    // on the first handshake — prime it now. Consumes the local `context`.
    rustls_platform_verifier::android::init_with_env(env, context)?;

    INSTALLED.store(true, Ordering::Release);
    tracing::info!("android JNI context installed (rustls-platform-verifier)");
    Ok(())
}
