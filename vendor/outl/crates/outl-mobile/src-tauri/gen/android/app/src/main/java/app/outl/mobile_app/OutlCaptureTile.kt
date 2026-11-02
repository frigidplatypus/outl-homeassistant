package app.outl.mobile_app

import android.app.PendingIntent
import android.os.Build
import android.service.quicksettings.TileService

/**
 * Quick settings tile (RFC 0363, the Android shape of the widget): one
 * pull-down and a tap opens the capture sheet.
 *
 * Like the widget, it writes nothing — it opens `outl://capture` and the
 * app does the rest. It is the closest Android has to iOS's Lock Screen
 * widget: reachable without unlocking the launcher, two gestures from
 * anywhere.
 */
class OutlCaptureTile : TileService() {
  override fun onClick() {
    super.onClick()
    val intent = OutlCapture.captureIntent()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      // `startActivityAndCollapse(Intent)` throws from API 34; the
      // PendingIntent overload is the replacement.
      startActivityAndCollapse(
        PendingIntent.getActivity(
          this,
          0,
          intent,
          PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        ),
      )
    } else {
      @Suppress("DEPRECATION")
      startActivityAndCollapse(intent)
    }
  }
}
