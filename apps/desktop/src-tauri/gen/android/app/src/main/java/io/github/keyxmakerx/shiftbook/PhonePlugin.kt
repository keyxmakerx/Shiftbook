package io.github.keyxmakerx.shiftbook

import android.app.Activity
import android.net.Uri
import android.provider.OpenableColumns
import android.view.WindowManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

/**
 * Shiftbook's own bits of Android (round 15), called from Rust (src/phone.rs), never from the
 * page. Screenshot blocking, and copying backups to and from the files the user picks.
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
}
