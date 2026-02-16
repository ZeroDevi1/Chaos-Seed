package com.zerodevi1.chaos_seed.core.ffi

import com.sun.jna.Native

object ChaosFfi {
    private val loaded: Lazy<Result<ChaosFfiApi>> = lazy {
        runCatching {
            // Library name should match `libchaos_ffi.so` in jniLibs.
            Native.load("chaos_ffi", ChaosFfiApi::class.java)
        }
    }

    fun api(): ChaosFfiApi {
        return loaded.value.getOrElse { e ->
            throw ChaosFfiException("Failed to load chaos_ffi native library", lastErrorJson = null, cause = e)
        }
    }
}

