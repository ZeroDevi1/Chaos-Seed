package com.zerodevi1.chaos_seed.ui.theme

import android.app.Activity
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.zerodevi1.chaos_seed.settings.AppThemeMode

@Composable
fun ChaosSeedTheme(
    themeMode: AppThemeMode,
    content: @Composable () -> Unit,
) {
    val ctx = LocalContext.current
    val sysDark = isSystemInDarkTheme()
    val dark = when (themeMode) {
        AppThemeMode.System -> sysDark
        AppThemeMode.Dark -> true
        AppThemeMode.Light -> false
    }

    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && dark -> dynamicDarkColorScheme(ctx)
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && !dark -> dynamicLightColorScheme(ctx)
        dark -> darkColorScheme()
        else -> lightColorScheme()
    }

    SystemBarsEffect(dark = dark)

    MaterialTheme(
        colorScheme = scheme,
        content = content,
    )
}

@Composable
private fun SystemBarsEffect(dark: Boolean) {
    val context = LocalContext.current
    val activity = context as? Activity ?: return
    val window = activity.window ?: return

    DisposableEffect(dark) {
        // Ensure edge-to-edge is enabled at the window level.
        WindowCompat.setDecorFitsSystemWindows(window, false)

        window.statusBarColor = Color.Transparent.toArgb()
        window.navigationBarColor = Color.Transparent.toArgb()

        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.isAppearanceLightStatusBars = !dark
        controller.isAppearanceLightNavigationBars = !dark

        onDispose { }
    }
}

