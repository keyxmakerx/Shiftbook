package io.github.keyxmakerx.shiftbook

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
  }
}
