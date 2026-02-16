package com.zerodevi1.chaos_seed.core.ffi

import com.sun.jna.Pointer

object FfiStrings {
    fun takeUtf8(ptr: Pointer?): String {
        if (ptr == null) {
            val last = runCatching { ChaosFfi.api().chaos_ffi_last_error_json() }.getOrNull()
            val lastStr = last?.getString(0, Charsets.UTF_8.name())
            // best-effort free the last error too
            runCatching { ChaosFfi.api().chaos_ffi_string_free(last) }
            throw ChaosFfiException("FFI returned null pointer", lastErrorJson = lastStr)
        }
        try {
            return ptr.getString(0, Charsets.UTF_8.name())
        } finally {
            runCatching { ChaosFfi.api().chaos_ffi_string_free(ptr) }
        }
    }
}

