package io.github.keyxmakerx.shiftbook

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.provider.OpenableColumns
import android.service.quicksettings.TileService
import android.view.WindowManager
import androidx.core.app.ActivityCompat
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

/**
 * Shiftbook's own bits of Android (round 15), called from Rust (src/phone.rs), never from the
 * page. Screenshot blocking, copying backups to and from the files the user picks, why the
 * app was opened, time in the background, the timer notification and tile, and nudges.
 */
@TauriPlugin
class PhonePlugin(private val activity: Activity) : Plugin(activity) {

    /** Block or allow screenshots, screen recording and the recent-apps preview. */
    @Command
    fun setSecure(invoke: Invoke) {
        val on = invoke.getArgs().getBoolean("on")
        activity.runOnUiThread {
            if (on) {
                activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
            invoke.resolve()
        }
    }

    /** Copy a file Shiftbook made (in its own folders) into a file the user picked. */
    @Command
    fun copyToUri(invoke: Invoke) {
        val args = invoke.getArgs()
        try {
            val uri = Uri.parse(args.getString("uri"))
            File(args.getString("path")).inputStream().use { input ->
                val out = activity.contentResolver.openOutputStream(uri, "wt")
                    ?: throw IllegalStateException("can't write there")
                out.use { input.copyTo(it) }
            }
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("copy_failed", e)
        }
    }

    /** Copy a file the user picked into Shiftbook's own folders. */
    @Command
    fun copyFromUri(invoke: Invoke) {
        val args = invoke.getArgs()
        try {
            val uri = Uri.parse(args.getString("uri"))
            val input = activity.contentResolver.openInputStream(uri)
                ?: throw IllegalStateException("can't read that")
            input.use { inp -> File(args.getString("path")).outputStream().use { inp.copyTo(it) } }
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("copy_failed", e)
        }
    }

    /** The name a picked file shows in the Files app. */
    @Command
    fun displayName(invoke: Invoke) {
        val uri = Uri.parse(invoke.getArgs().getString("uri"))
        var name = ""
        try {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { c -> if (c.moveToFirst()) name = c.getString(0) ?: "" }
        } catch (_: Exception) {
        }
        val ret = JSObject()
        ret.put("name", name)
        invoke.resolve(ret)
    }

    /** Why Shiftbook was opened (shared text, a shortcut, the tile, a nudge), once. */
    @Command
    fun takeLaunch(invoke: Invoke) {
        val ret = JSObject()
        PhoneState.takeLaunch()?.let { (kind, text) ->
            ret.put("kind", kind)
            if (text != null) ret.put("text", text)
        }
        invoke.resolve(ret)
    }

    /** Seconds in the background since the timer went into the notification. */
    @Command
    fun takeAway(invoke: Invoke) {
        val ret = JSObject()
        ret.put("seconds", PhoneState.takeAwaySeconds())
        invoke.resolve(ret)
    }

    /** Show (or take away) the running timer in the notification and the tile. */
    @Command
    fun timerAway(invoke: Invoke) {
        val args = invoke.getArgs()
        val on = args.getBoolean("on")
        val seconds = args.getLong("seconds")
        if (on) {
            PhoneState.startAway()
            PhoneState.timerBase = SystemClock.elapsedRealtime() - seconds * 1000
            Nudges.showTimer(activity, seconds)
        } else {
            PhoneState.timerBase = 0
            Nudges.hideTimer(activity)
        }
        try {
            TileService.requestListeningState(activity, ComponentName(activity, TimerTile::class.java))
        } catch (_: Exception) {
        }
        invoke.resolve()
    }

    /** May Shiftbook notify? From Android 13 this asks the user the first time. */
    @Command
    fun allowNotifications(invoke: Invoke) {
        val allowed = Nudges.allowed(activity)
        if (!allowed && Build.VERSION.SDK_INT >= 33) {
            ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.POST_NOTIFICATIONS), 7)
        }
        val ret = JSObject()
        ret.put("allowed", allowed)
        invoke.resolve(ret)
    }

    /** Replace the planned nudges (worked out by the app) with these. */
    @Command
    fun planNudges(invoke: Invoke) {
        try {
            Nudges.plan(activity, invoke.getArgs().getJSONArray("nudges"))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("plan_failed", e)
        }
    }
}
