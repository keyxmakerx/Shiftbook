package io.github.keyxmakerx.shiftbook

import android.app.PendingIntent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/**
 * The Shiftbook timer tile in the pull-down (round 15, idea F). Shows whether the timer runs,
 * never what it's on. A tap unlocks the phone if needed and opens Shiftbook, where the timer
 * is stopped or started as usual.
 */
class TimerTile : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        val tile = qsTile ?: return
        val running = PhoneState.timerBase > 0
        tile.state = if (running) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "Shiftbook timer"
        if (Build.VERSION.SDK_INT >= 29) tile.subtitle = if (running) "Running" else "Off"
        tile.updateTile()
    }

    override fun onClick() {
        super.onClick()
        if (isLocked) unlockAndRun { open() } else open()
    }

    private fun open() {
        val intent = PhoneState.openIntent(this, "timer")
        if (Build.VERSION.SDK_INT >= 34) {
            startActivityAndCollapse(
                PendingIntent.getActivity(this, 40, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            )
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(intent)
        }
    }
}
