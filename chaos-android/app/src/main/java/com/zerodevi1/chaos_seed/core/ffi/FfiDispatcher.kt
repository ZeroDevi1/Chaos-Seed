package com.zerodevi1.chaos_seed.core.ffi

import kotlinx.coroutines.CoroutineDispatcher
import java.util.concurrent.Executors
import kotlinx.coroutines.asCoroutineDispatcher

object FfiDispatcher {
    // Single threaded dispatcher to mirror Flutter's isolate semantics (avoid FFI thread-safety pitfalls).
    val dispatcher: CoroutineDispatcher =
        Executors.newSingleThreadExecutor { r -> Thread(r, "chaos-ffi").apply { isDaemon = true } }
            .asCoroutineDispatcher()
}

