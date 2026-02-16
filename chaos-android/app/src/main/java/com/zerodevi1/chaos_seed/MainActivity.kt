package com.zerodevi1.chaos_seed

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.core.view.WindowCompat
import com.zerodevi1.chaos_seed.ui.ChaosSeedApp
import com.zerodevi1.chaos_seed.ui.theme.ChaosSeedTheme
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import com.zerodevi1.chaos_seed.settings.SettingsViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)

        setContent {
            val settingsVm: SettingsViewModel = viewModel()
            val s by settingsVm.state.collectAsState()

            ChaosSeedTheme(themeMode = s.themeMode) {
                ChaosSeedApp()
            }
        }
    }
}
