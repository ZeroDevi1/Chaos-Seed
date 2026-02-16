package com.zerodevi1.chaos_seed.player

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

interface PlayerSessionController {
    fun requestSwitchEngine(type: PlayerEngineType)
}

object PlayerSessionRegistry {
    private val _isActive = MutableStateFlow(false)
    val isActive: StateFlow<Boolean> = _isActive

    @Volatile
    var controller: PlayerSessionController? = null
        private set

    fun register(controller: PlayerSessionController) {
        this.controller = controller
        _isActive.value = true
    }

    fun unregister(controller: PlayerSessionController) {
        if (this.controller === controller) {
            this.controller = null
            _isActive.value = false
        }
    }
}

