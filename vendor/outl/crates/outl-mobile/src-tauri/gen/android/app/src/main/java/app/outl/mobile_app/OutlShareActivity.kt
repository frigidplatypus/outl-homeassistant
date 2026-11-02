package app.outl.mobile_app

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.widget.Toast

/**
 * Share target (RFC 0363): text or a link shared to outl becomes one
 * line in the capture inbox and lands in the journal for the day it was
 * shared the next time the app is opened.
 *
 * The Android counterpart of iOS's `OutlShare` extension, and the same
 * contract: no workspace access, no UI beyond a short confirmation. It
 * is an ordinary activity rather than an extension because Android has
 * no extension model — which is also why it can write through Rust (see
 * [OutlCapture]).
 *
 * Invisible on purpose: a translucent theme, no history, out of recents
 * (see the manifest). The user stays in the app they were sharing from,
 * which is the whole point of sharing rather than switching.
 */
class OutlShareActivity : Activity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    val saved = handle(intent)
    Toast.makeText(
      this,
      if (saved) R.string.capture_saved else R.string.capture_nothing_to_save,
      Toast.LENGTH_SHORT,
    ).show()
    finish()
  }

  /**
   * `false` when nothing was saved, which covers an empty share, a
   * different action, and a failed write. The composition of text + URL
   * into one line belongs to `outl_actions::capture::shared_line`, so
   * both values go across as they arrived.
   */
  private fun handle(intent: Intent?): Boolean {
    if (intent == null || intent.action != Intent.ACTION_SEND) return false
    val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
    // Chrome and most readers put the page title in EXTRA_SUBJECT and
    // the link in EXTRA_TEXT. A title with no link is nothing to save on
    // its own, so it is only ever the text half of the pair.
    val subject = intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT)?.toString()
    val (body, url) = if (subject.isNullOrBlank()) text to null else subject to text
    return OutlCapture.save(this, body, url, SOURCE)
  }

  private companion object {
    /** Matches iOS's `CaptureSource.share`, which the drain logs. */
    const val SOURCE = "share"
  }
}
