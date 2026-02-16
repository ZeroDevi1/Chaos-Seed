package com.zerodevi1.chaos_seed.core.storage

import android.content.Context
import android.os.Environment
import java.io.File

object AndroidDownloadDir {
    /**
     * Best-effort:
     * 1) Try public Downloads/ChaosSeed (may fail due to scoped storage policies on newer Android).
     * 2) Fallback to app-scoped external files dir.
     */
    fun pickWritableDir(context: Context): String {
        val public = runCatching {
            val base = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
            File(base, "ChaosSeed")
        }.getOrNull()

        if (public != null && canWriteDir(public)) return public.absolutePath

        val scoped = File(
            context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS) ?: context.filesDir,
            "ChaosSeed",
        )
        scoped.mkdirs()
        return scoped.absolutePath
    }

    private fun canWriteDir(dir: File): Boolean {
        return runCatching {
            dir.mkdirs()
            if (!dir.exists() || !dir.isDirectory) return false
            val probe = File(dir, ".probe_${System.nanoTime()}.tmp")
            probe.writeText("ok")
            probe.delete()
            true
        }.getOrElse { false }
    }
}

