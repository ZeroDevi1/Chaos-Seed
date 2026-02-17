package com.zerodevi1.chaos_seed.player

import android.app.PictureInPictureParams
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.os.Build
import android.os.Bundle
import android.content.res.Configuration
import android.util.Rational
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.view.WindowCompat
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.zerodevi1.chaos_seed.player.ui.PlayerScreen
import com.zerodevi1.chaos_seed.ui.theme.ChaosSeedTheme
import com.zerodevi1.chaos_seed.settings.SettingsViewModel
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.collect

class PlayerActivity : ComponentActivity(), PlayerSessionController {
    private val vm: PlayerViewModel by viewModels()

    private var pipMode by mutableStateOf(false)
    private var lastOrientation: Int = Configuration.ORIENTATION_UNDEFINED

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        lastOrientation = resources.configuration.orientation

        PlayerSessionRegistry.register(this)

        val input = intent.getStringExtra(EXTRA_INPUT)?.trim()
        val initialVariantId = intent.getStringExtra(EXTRA_INITIAL_VARIANT_ID)?.trim()
        val rawUrl = intent.getStringExtra(EXTRA_URL).orEmpty().trim()
        if (!input.isNullOrBlank()) {
            vm.startLive(input = input, initialVariantId = initialVariantId)
        } else {
            val url = rawUrl.ifBlank { SAMPLE_HLS_URL }
            vm.open(url = url, headers = emptyMap())
        }

        // Keep PiP params in sync (ratio + auto-enter on API 31+).
        if (Build.VERSION.SDK_INT >= 26) {
            lifecycleScope.launch {
                repeatOnLifecycle(Lifecycle.State.STARTED) {
                    vm.state.collect { s ->
                        val w = s.videoWidth.coerceAtLeast(1)
                        val h = s.videoHeight.coerceAtLeast(1)
                        val ratio = if (h > w) Rational(9, 16) else Rational(16, 9)
                        val b = PictureInPictureParams.Builder().setAspectRatio(ratio)
                        if (Build.VERSION.SDK_INT >= 31) b.setAutoEnterEnabled(true)
                        setPictureInPictureParams(b.build())
                    }
                }
            }
        }

        setContent {
            val sVm: SettingsViewModel = viewModel()
            val s by sVm.state.collectAsState()
            ChaosSeedTheme(themeMode = s.themeMode) {
                PlayerScreen(
                    vm = vm,
                    pipMode = pipMode,
                    onBack = { finish() },
                    onEnterPip = { enterPip() },
                    onToggleOrientation = { toggleOrientation() },
                )
            }
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        if (newConfig.orientation != lastOrientation) {
            // Detach ASAP so mpv stops queueing buffers to the old Surface/BufferQueue during rotation.
            vm.detachSurface()
            lastOrientation = newConfig.orientation
        }
        super.onConfigurationChanged(newConfig)
    }

    override fun onDestroy() {
        PlayerSessionRegistry.unregister(this)
        super.onDestroy()
    }

    override fun onPictureInPictureModeChanged(
        isInPictureInPictureMode: Boolean,
        newConfig: Configuration,
    ) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        pipMode = isInPictureInPictureMode
        vm.onPipModeChanged(isInPictureInPictureMode)
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        // API 31+ can auto-enter; for older versions we best-effort enter on leave if playing.
        if (Build.VERSION.SDK_INT in 26..30) {
            if (vm.state.value.playing) {
                enterPip()
            }
        }
    }

    private fun enterPip() {
        if (Build.VERSION.SDK_INT < 26) return

        val s = vm.state.value
        val w = s.videoWidth.coerceAtLeast(1)
        val h = s.videoHeight.coerceAtLeast(1)
        val ratio = if (h > w) Rational(9, 16) else Rational(16, 9)

        val b = PictureInPictureParams.Builder()
            .setAspectRatio(ratio)

        if (Build.VERSION.SDK_INT >= 31) {
            b.setAutoEnterEnabled(true)
        }

        val params = b.build()
        setPictureInPictureParams(params)
        enterPictureInPictureMode(params)
    }

    private fun toggleOrientation() {
        val o = resources.configuration.orientation
        requestedOrientation =
            if (o == Configuration.ORIENTATION_PORTRAIT) {
                ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            } else {
                ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
            }
    }

    override fun requestSwitchEngine(type: PlayerEngineType) {
        vm.switchEngine(type)
    }

    companion object {
        const val EXTRA_URL = "extra.url"
        const val EXTRA_INPUT = "extra.input"
        const val EXTRA_INITIAL_VARIANT_ID = "extra.initialVariantId"

        // Used for quick local validation only.
        private const val SAMPLE_HLS_URL = "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8"

        fun intentForLive(context: Context, input: String, initialVariantId: String?): Intent {
            return Intent(context, PlayerActivity::class.java).apply {
                putExtra(EXTRA_INPUT, input)
                val v = initialVariantId?.trim().orEmpty()
                if (v.isNotEmpty()) putExtra(EXTRA_INITIAL_VARIANT_ID, v)
            }
        }
    }
}
