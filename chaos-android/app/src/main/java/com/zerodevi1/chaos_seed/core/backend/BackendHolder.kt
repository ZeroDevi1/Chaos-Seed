package com.zerodevi1.chaos_seed.core.backend

import android.content.Context

/**
 * Keep a single backend instance for the whole process so Activity hops (e.g. LiveDecode -> PlayerActivity)
 * can reuse caches (manifest) and avoid repeated FFI init work.
 */
object BackendHolder {
    @Volatile
    private var backend: ChaosBackend? = null

    fun get(appContext: Context): ChaosBackend {
        val existing = backend
        if (existing != null) return existing
        return synchronized(this) {
            backend ?: FfiChaosBackend(appContext.applicationContext).also { backend = it }
        }
    }
}

