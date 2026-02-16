package com.zerodevi1.chaos_seed.core.backend

import androidx.compose.runtime.staticCompositionLocalOf

val LocalBackend = staticCompositionLocalOf<ChaosBackend> {
    error("LocalBackend not provided")
}

