package io.github.keyxmakerx.shiftbook

import android.content.Context
import android.content.Intent
import android.os.SystemClock

/**
 * What the Android side remembers while Shiftbook's process lives (round 15). Nothing here is
 * saved: why the app was opened, how long it was in the background, and whether a timer is
 * showing in the notification and the tile. Never a title, project or ticket.
 */
object PhoneState {
    /** The intent action for opening Shiftbook from a shortcut, the tile or a nudge. */
    const val ACTION_OPEN = "io.github.keyxmakerx.shiftbook.OPEN"
    const val EXTRA_KIND = "kind"
    private const val MAX_SHARED_CHARS = 10_000

    @Volatile private var launchKind: String? = null
    @Volatile private var launchText: String? = null

    @Volatile private var pausedAt = 0L
    @Volatile private var awayMillis = 0L

    /** `SystemClock.elapsedRealtime()` when the timer would have started; 0 when none shows. */
    @Volatile var timerBase = 0L

    /** Note why Shiftbook was opened (MainActivity, on create and on a new intent). */
    fun noteIntent(intent: Intent?) {
        intent ?: return
        when (intent.action) {
            Intent.ACTION_SEND -> {
                if (intent.type?.startsWith("text/") != true) return
                val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT).orEmpty().trim()
                val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString().orEmpty().trim()
                val both = listOf(subject, text).filter { it.isNotEmpty() }.joinToString("\n")
                if (both.isEmpty()) return
                launchKind = "share"
                launchText = both.take(MAX_SHARED_CHARS)
            }
            ACTION_OPEN -> {
                launchKind = intent.getStringExtra(EXTRA_KIND) ?: return
                launchText = null
            }
        }
    }

    /** Why Shiftbook was opened, once. */
    fun takeLaunch(): Pair<String, String?>? {
        val kind = launchKind ?: return null
        val text = launchText
        launchKind = null
        launchText = null
        return Pair(kind, text)
    }

    fun paused() {
        pausedAt = SystemClock.elapsedRealtime()
    }

    fun resumed() {
        if (pausedAt > 0) awayMillis += SystemClock.elapsedRealtime() - pausedAt
        pausedAt = 0
    }

    /** Time in the background since the timer went into the notification, then forgotten. */
    fun takeAwaySeconds(): Long {
        val s = awayMillis / 1000
        awayMillis = 0
        return s
    }

    fun startAway() {
        awayMillis = 0
    }

    /** An intent that opens Shiftbook for `kind` (shortcuts use the same action in XML). */
    fun openIntent(context: Context, kind: String): Intent =
        Intent(context, MainActivity::class.java)
            .setAction(ACTION_OPEN)
            .putExtra(EXTRA_KIND, kind)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
}
