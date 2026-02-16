package com.zerodevi1.chaos_seed.core.ffi

class ChaosFfiException(
    message: String,
    val lastErrorJson: String?,
    cause: Throwable? = null,
) : RuntimeException(message, cause)

