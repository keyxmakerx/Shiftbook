package io.github.keyxmakerx.shiftbook

import android.content.Intent
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    // Protected from the first frame, like the desktop window (SECURITY.md). The Protected
    // pill turns it off for ten minutes through PhonePlugin.setSecure.
    window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
    super.onCreate(savedInstanceState)
    if (savedInstanceState == null) PhoneState.noteIntent(intent)
  }

  // Shared text, a long-press shortcut, the timer tile or a nudge, while already open.
  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    PhoneState.noteIntent(intent)
  }

  override fun onPause() {
    PhoneState.paused()
    super.onPause()
  }

  override fun onResume() {
    super.onResume()
    PhoneState.resumed()
  }
}
