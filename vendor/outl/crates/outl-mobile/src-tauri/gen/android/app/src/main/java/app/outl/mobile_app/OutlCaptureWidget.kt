package app.outl.mobile_app

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.widget.RemoteViews

/**
 * Home screen widget (RFC 0363 step 1): one tap opens the app on the
 * capture sheet.
 *
 * It writes nothing. The deep link does the work, so this needs no
 * inbox, no native library and no permissions — which is why the RFC
 * ships it first, and why the Android one is a few lines next to the
 * share target.
 */
class OutlCaptureWidget : AppWidgetProvider() {
  override fun onUpdate(
    context: Context,
    manager: AppWidgetManager,
    appWidgetIds: IntArray,
  ) {
    val views = RemoteViews(context.packageName, R.layout.outl_capture_widget)
    views.setOnClickPendingIntent(R.id.capture_widget_root, pendingCapture(context))
    for (id in appWidgetIds) {
      manager.updateAppWidget(id, views)
    }
  }

  private fun pendingCapture(context: Context): PendingIntent =
    PendingIntent.getActivity(
      context,
      0,
      OutlCapture.captureIntent(),
      // IMMUTABLE because nothing fills this intent in later, and a
      // mutable one would let another app rewrite where it points.
      // Required from API 31 regardless.
      PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
}
