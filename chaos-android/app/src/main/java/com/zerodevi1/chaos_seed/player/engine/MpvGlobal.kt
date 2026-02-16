package com.zerodevi1.chaos_seed.player.engine

import java.util.concurrent.atomic.AtomicBoolean

/**
 * MPVLib is effectively a singleton in this integration (static global native state).
 * Enforce one active instance in-process to avoid surface fights and native leaks.
 */
object MpvGlobal {
    private val acquired = AtomicBoolean(false)

    fun acquireOrThrow() {
        check(acquired.compareAndSet(false, true)) { "MPV is already in use (single-instance)" }
    }

    fun release() {
        acquired.set(false)
    }
}

