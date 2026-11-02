//! Android's side of the capture inbox (RFC 0363).
//!
//! The share target writes a capture the same way the iOS extensions do,
//! except it can do it through Rust: on Android the widget, the share
//! target and the quick settings tile are components of this app, in
//! this process, with this library already loaded. So
//! [`outl_actions::capture::write_capture`] stays the single owner of
//! the inbox file format here, and Kotlin only supplies the folder and
//! the text (`OutlCapture.kt`).
//!
//! That is the one place Android deliberately differs from iOS, where a
//! widget and a share extension are separate processes under a memory
//! ceiling this library would not fit, and a small Swift writer
//! (`OutlKit/Capture/CaptureInbox.swift`) owns the format instead.
//!
//! Nothing here touches a workspace. The capture waits in the inbox
//! until `drain_capture_inbox` turns it into ops, which is what keeps
//! root invariant 7 — a second process must never append to the op log.

/// Compose one line out of what the share sheet handed over and write
/// it into the inbox folder Kotlin passes in.
///
/// `text` and `url` are the two things an Android `ACTION_SEND` can
/// carry, and `outl_actions::capture::shared_line` is the one owner of
/// how they become a single line — so Kotlin holds neither that rule nor
/// the file format, only the folder and the strings. Either may be
/// empty; both empty is "nothing to save".
///
/// `JNI_FALSE` means nothing was published: blank text, an unreadable
/// argument, or a failed write. The caller shows that as "nothing
/// saved" rather than claiming a line it does not have — a capture that
/// silently disappears is the failure RFC 0363 exists to avoid.
///
/// Style follows `bg_sync.rs` and `android_jni.rs`: `EnvUnowned` +
/// `with_env` (which wraps the body in `catch_unwind`) +
/// [`jni::errors::LogErrorAndDefault`] (which logs, never throws). A
/// thrown exception would land inside the share activity and crash the
/// share sheet; a panic unwinding into the JVM would abort the process.
/// Both are worse answers than "could not save that".
///
/// # Safety
///
/// Called by the JVM with a valid JNI env and class, four non-null
/// `String` arguments (`dir`, `text`, `url`, `source`) and a `jlong`.
/// Kotlin's non-null signature on `OutlCapture.write` is what makes the
/// strings non-null, and only this app's own process can reach the
/// symbol.
#[no_mangle]
pub extern "system" fn Java_app_outl_mobile_1app_OutlCapture_write<'local>(
    mut unowned_env: jni::EnvUnowned<'local>,
    _class: jni::objects::JClass<'local>,
    dir: jni::objects::JString<'local>,
    text: jni::objects::JString<'local>,
    url: jni::objects::JString<'local>,
    source: jni::objects::JString<'local>,
    captured_at_ms: jni::sys::jlong,
) -> jni::sys::jboolean {
    unowned_env
        .with_env(|env: &mut jni::Env| {
            let dir = dir.try_to_string(env)?;
            let text = text.try_to_string(env)?;
            let url = url.try_to_string(env)?;
            let source = source.try_to_string(env)?;
            let Some(line) = outl_actions::capture::shared_line(Some(&text), Some(&url)) else {
                return Ok(jni::sys::JNI_FALSE);
            };
            let wrote = outl_actions::capture::write_capture(
                std::path::Path::new(&dir),
                &line,
                &source,
                captured_at_ms,
            );
            if let Err(e) = &wrote {
                tracing::warn!("capture inbox: refused a capture from {source}: {e}");
            }
            Ok::<_, jni::errors::Error>(if wrote.is_ok() {
                jni::sys::JNI_TRUE
            } else {
                jni::sys::JNI_FALSE
            })
        })
        .resolve::<jni::errors::LogErrorAndDefault>()
}
