package io.github.keyxmakerx.shiftbook

import android.Manifest
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject
import java.util.Calendar

/**
 * Notifications (round 15): the timer while Shiftbook is in the background, and the quiet
 * nudges (due reminder, wrap-up). The app works out every nudge and its words (Rust,
 * crates/core/src/nudge.rs); this only shows them. They hold counts and times, never titles,
 * so they are safe on the lock screen.
 */
object Nudges {
    private const val CHANNEL_TIMER = "timer"
    private const val CHANNEL_NUDGES = "nudges"
    private const val ID_TIMER = 1
    private const val PREFS = "nudges"
    private const val KEY_PLANNED = "planned"
    private val KINDS = listOf("due", "wrap_up")

    private fun channels(context: Context) {
        if (Build.VERSION.SDK_INT < 26) return
        val nm = context.getSystemService(NotificationManager::class.java) ?: return
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_TIMER, "Timer", NotificationManager.IMPORTANCE_LOW).apply {
                description = "Shows that the timer runs while Shiftbook is in the background."
                setShowBadge(false)
            }
        )
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_NUDGES, "Reminders", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "The due reminder and the wrap-up nudge. Counts only, never titles."
            }
        )
    }

    fun allowed(context: Context): Boolean {
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) return false
        return NotificationManagerCompat.from(context).areNotificationsEnabled()
    }

    private fun opener(context: Context, kind: String, code: Int): PendingIntent =
        PendingIntent.getActivity(
            context,
            code,
            PhoneState.openIntent(context, kind),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

    /** The timer is running while Shiftbook is in the background: how long, never on what. */
    fun showTimer(context: Context, seconds: Long) {
        if (!allowed(context)) return
        channels(context)
        val n = NotificationCompat.Builder(context, CHANNEL_TIMER)
            .setSmallIcon(R.drawable.ic_stat_shiftbook)
            .setContentTitle("Timer running")
            .setContentText("Open Shiftbook to stop it.")
            .setWhen(System.currentTimeMillis() - seconds * 1000)
            .setShowWhen(true)
            .setUsesChronometer(true)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setContentIntent(opener(context, "timer", 10))
            .build()
        try {
            NotificationManagerCompat.from(context).notify(ID_TIMER, n)
        } catch (_: SecurityException) {
        }
    }

    fun hideTimer(context: Context) {
        NotificationManagerCompat.from(context).cancel(ID_TIMER)
    }

    private fun receiverIntent(context: Context, kind: String): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            20 + KINDS.indexOf(kind).coerceAtLeast(0),
            Intent(context, NudgeReceiver::class.java).setAction("io.github.keyxmakerx.shiftbook.NUDGE.$kind"),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

    /** Local wall-clock time for `YYYY-MM-DD` and minutes after midnight. */
    private fun millisAt(date: String, at: Int): Long? {
        val parts = date.split("-").mapNotNull { it.toIntOrNull() }
        if (parts.size != 3) return null
        return Calendar.getInstance().apply {
            clear()
            set(parts[0], parts[1] - 1, parts[2], at / 60, at % 60, 0)
        }.timeInMillis
    }

    /** Replace every planned nudge with these (each `{kind, date, at, title, body}`). */
    fun plan(context: Context, nudges: JSONArray) {
        val alarms = context.getSystemService(AlarmManager::class.java) ?: return
        for (kind in KINDS) alarms.cancel(receiverIntent(context, kind))
        val kept = JSONArray()
        val now = System.currentTimeMillis()
        for (i in 0 until nudges.length()) {
            val n = nudges.optJSONObject(i) ?: continue
            val kind = n.optString("kind")
            if (kind !in KINDS) continue
            val at = millisAt(n.optString("date"), n.optInt("at", -1).takeIf { it in 0..1439 } ?: continue) ?: continue
            if (at <= now) continue
            n.put("millis", at)
            kept.put(n)
            // Within ten minutes of the time; needs no exact-alarm permission.
            alarms.setWindow(AlarmManager.RTC_WAKEUP, at, 10 * 60 * 1000L, receiverIntent(context, kind))
        }
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY_PLANNED, kept.toString()).apply()
    }

    /** After a restart of the phone: plan again what was planned. */
    fun replan(context: Context) {
        val saved = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_PLANNED, null) ?: return
        val list = try { JSONArray(saved) } catch (_: Exception) { return }
        plan(context, list)
    }

    /** A planned nudge is due: show it, then forget it. */
    fun fire(context: Context, kind: String) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val list = try { JSONArray(prefs.getString(KEY_PLANNED, "[]")) } catch (_: Exception) { JSONArray() }
        var found: JSONObject? = null
        val left = JSONArray()
        for (i in 0 until list.length()) {
            val n = list.optJSONObject(i) ?: continue
            if (found == null && n.optString("kind") == kind) found = n else left.put(n)
        }
        prefs.edit().putString(KEY_PLANNED, left.toString()).apply()
        val n = found ?: return
        if (!allowed(context)) return
        channels(context)
        val note = NotificationCompat.Builder(context, CHANNEL_NUDGES)
            .setSmallIcon(R.drawable.ic_stat_shiftbook)
            .setContentTitle(n.optString("title"))
            .setContentText(n.optString("body"))
            .setAutoCancel(true)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setContentIntent(opener(context, if (kind == "due") "due" else "wrapup", 30 + KINDS.indexOf(kind)))
            .build()
        try {
            NotificationManagerCompat.from(context).notify(100 + KINDS.indexOf(kind), note)
        } catch (_: SecurityException) {
        }
    }
}

/** Shows a planned nudge when its alarm goes off. */
class NudgeReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val kind = intent.action?.substringAfterLast('.') ?: return
        Nudges.fire(context, kind)
    }
}

/** Alarms are lost when the phone restarts; plan the saved ones again. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) Nudges.replan(context)
    }
}
