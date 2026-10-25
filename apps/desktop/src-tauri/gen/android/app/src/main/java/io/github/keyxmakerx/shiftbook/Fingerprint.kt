package io.github.keyxmakerx.shiftbook

import android.app.Activity
import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import android.provider.Settings
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.annotation.RequiresApi
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Fingerprint unlock (round 15), off until turned on in Security. Turning it on encrypts the
 * password with a key in the phone's secure hardware that only opens after a fingerprint
 * (strong biometrics; adding a new fingerprint wipes the key). After the phone restarts the
 * password is asked for once, like a banking app. Uses Android's own BiometricPrompt, so
 * Android 11 or newer; no extra packages. The password goes to Rust, never to the page.
 */
object Fingerprint {
    private const val KEY = "shiftbook_fingerprint"
    private const val PREFS = "fingerprint"
    private const val DATA = "data"
    private const val IV = "iv"
    private const val BOOT = "boot"

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun available(context: Context): Boolean {
        if (Build.VERSION.SDK_INT < 30) return false
        val bm = context.getSystemService(BiometricManager::class.java) ?: return false
        return bm.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG) == BiometricManager.BIOMETRIC_SUCCESS
    }

    fun isOn(context: Context): Boolean = prefs(context).contains(DATA)

    private fun bootCount(context: Context): Int =
        Settings.Global.getInt(context.contentResolver, Settings.Global.BOOT_COUNT, -1)

    /** On, and the password was typed since the phone last started. */
    fun ready(context: Context): Boolean =
        available(context) && isOn(context) && prefs(context).getInt(BOOT, -2) == bootCount(context)

    /** The password was just typed: fingerprint works until the next restart. */
    fun noteBoot(context: Context) {
        if (isOn(context)) prefs(context).edit().putInt(BOOT, bootCount(context)).apply()
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    fun off(context: Context) {
        prefs(context).edit().clear().apply()
        try {
            keyStore().deleteEntry(KEY)
        } catch (_: Exception) {
        }
    }

    @RequiresApi(30)
    private fun newKey(): SecretKey {
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(KEY, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .setUserAuthenticationRequired(true)
                .setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
                .setInvalidatedByBiometricEnrollment(true)
                .build()
        )
        return gen.generateKey()
    }

    /** Ask for a fingerprint, then store the password encrypted. */
    fun turnOn(activity: Activity, password: String, done: (Boolean) -> Unit) {
        if (Build.VERSION.SDK_INT < 30 || !available(activity)) return done(false)
        off(activity)
        val cipher = try {
            Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, newKey()) }
        } catch (_: Exception) {
            return done(false)
        }
        prompt(activity, "Turn on fingerprint unlock", cipher) { c ->
            if (c == null) return@prompt done(false)
            try {
                val data = c.doFinal(password.toByteArray(Charsets.UTF_8))
                prefs(activity).edit()
                    .putString(DATA, Base64.encodeToString(data, Base64.NO_WRAP))
                    .putString(IV, Base64.encodeToString(c.iv, Base64.NO_WRAP))
                    .putInt(BOOT, bootCount(activity))
                    .apply()
                done(true)
            } catch (_: Exception) {
                off(activity)
                done(false)
            }
        }
    }

    /** Ask for a fingerprint, then give back the password (null if not, or it can't). */
    fun unlock(activity: Activity, done: (String?) -> Unit) {
        if (Build.VERSION.SDK_INT < 30 || !ready(activity)) return done(null)
        val p = prefs(activity)
        val cipher = try {
            val key = keyStore().getKey(KEY, null) as? SecretKey ?: throw IllegalStateException()
            val iv = Base64.decode(p.getString(IV, ""), Base64.NO_WRAP)
            Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv)) }
        } catch (e: KeyPermanentlyInvalidatedException) {
            // A fingerprint was added or removed: turn it off; the password still works.
            off(activity)
            return done(null)
        } catch (_: Exception) {
            return done(null)
        }
        prompt(activity, "Unlock Shiftbook", cipher) { c ->
            if (c == null) return@prompt done(null)
            try {
                val data = Base64.decode(p.getString(DATA, ""), Base64.NO_WRAP)
                done(String(c.doFinal(data), Charsets.UTF_8))
            } catch (_: Exception) {
                done(null)
            }
        }
    }

    @RequiresApi(30)
    private fun prompt(activity: Activity, title: String, cipher: Cipher, then: (Cipher?) -> Unit) {
        var answered = false
        val once: (Cipher?) -> Unit = { c ->
            if (!answered) {
                answered = true
                then(c)
            }
        }
        activity.runOnUiThread {
            val executor = activity.mainExecutor
            val prompt = BiometricPrompt.Builder(activity)
                .setTitle(title)
                .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
                .setNegativeButton("Use password", executor) { _, _ -> once(null) }
                .build()
            prompt.authenticate(
                BiometricPrompt.CryptoObject(cipher),
                CancellationSignal(),
                executor,
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        once(result.cryptoObject?.cipher)
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        once(null)
                    }
                }
            )
        }
    }
}
