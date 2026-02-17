package com.zerodevi1.chaos_seed.core.storage

import android.content.ContentUris
import android.content.ContentValues
import android.content.Context
import android.media.MediaScannerConnection
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File

object PublicDownloadsExporter {
    data class Exported(
        val uri: Uri?,
        // Human-readable path hint for UI (e.g. "Downloads/ChaosSeed/A/B/file.mp3")
        val displayPath: String,
        val skipped: Boolean,
    )

    fun exportIntoChaosSeedDownloads(
        context: Context,
        outDir: File,
        source: File,
        overwrite: Boolean = false,
    ): Exported {
        require(source.exists() && source.isFile) { "source is not a file: ${source.absolutePath}" }

        val rel = runCatching { source.relativeTo(outDir).invariantSeparatorsPath }.getOrNull()
        val relParent = rel?.substringBeforeLast('/', missingDelimiterValue = "")?.trim('/').orEmpty()
        val relDir = buildString {
            append(Environment.DIRECTORY_DOWNLOADS)
            append("/ChaosSeed")
            if (relParent.isNotEmpty()) {
                append("/")
                append(relParent)
            }
            append("/")
        }

        val displayPath = buildString {
            append("Downloads/ChaosSeed")
            if (relParent.isNotEmpty()) {
                append("/")
                append(relParent)
            }
            append("/")
            append(source.name)
        }

        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            exportViaMediaStore(context, source, relDir, displayPath, overwrite)
        } else {
            exportViaFileSystem(context, source, relParent, displayPath, overwrite)
        }
    }

    private fun exportViaMediaStore(
        context: Context,
        source: File,
        relativeDir: String,
        displayPath: String,
        overwrite: Boolean,
    ): Exported {
        val mimeType = guessMimeType(source.name)
        val resolver = context.contentResolver

        val existing = findExistingDownloadsItem(resolver, relativeDir, source.name)
        if (existing != null && !overwrite) {
            return Exported(uri = existing, displayPath = displayPath, skipped = true)
        }
        if (existing != null && overwrite) {
            runCatching { resolver.delete(existing, null, null) }
        }

        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, source.name)
            if (mimeType != null) put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
            put(MediaStore.MediaColumns.RELATIVE_PATH, relativeDir)
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }

        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: throw IllegalStateException("failed to insert into MediaStore downloads")

        try {
            resolver.openOutputStream(uri, "w").use { out ->
                requireNotNull(out) { "failed to open output stream" }
                source.inputStream().use { input -> input.copyTo(out) }
            }

            val done = ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }
            resolver.update(uri, done, null, null)
            return Exported(uri = uri, displayPath = displayPath, skipped = false)
        } catch (e: Exception) {
            runCatching { resolver.delete(uri, null, null) }
            throw e
        }
    }

    private fun exportViaFileSystem(
        context: Context,
        source: File,
        relParent: String,
        displayPath: String,
        overwrite: Boolean,
    ): Exported {
        val base = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
        val destDir = if (relParent.isEmpty()) {
            File(base, "ChaosSeed")
        } else {
            File(File(base, "ChaosSeed"), relParent)
        }
        destDir.mkdirs()
        val dest = File(destDir, source.name)

        if (dest.exists() && !overwrite) {
            return Exported(uri = Uri.fromFile(dest), displayPath = displayPath, skipped = true)
        }
        if (dest.exists() && overwrite) {
            runCatching { dest.delete() }
        }

        source.copyTo(dest, overwrite = true)
        MediaScannerConnection.scanFile(context, arrayOf(dest.absolutePath), null, null)
        return Exported(uri = Uri.fromFile(dest), displayPath = displayPath, skipped = false)
    }

    private fun guessMimeType(fileName: String): String? {
        val ext = fileName.substringAfterLast('.', missingDelimiterValue = "").lowercase()
        return when (ext) {
            "mp3" -> "audio/mpeg"
            "m4a" -> "audio/mp4"
            "aac" -> "audio/aac"
            "flac" -> "audio/flac"
            "wav" -> "audio/wav"
            "ogg" -> "audio/ogg"
            "lrc", "txt" -> "text/plain"
            else -> null
        }
    }

    private fun findExistingDownloadsItem(
        resolver: android.content.ContentResolver,
        relativeDir: String,
        displayName: String,
    ): Uri? {
        val projection = arrayOf(MediaStore.MediaColumns._ID)
        val selection = "${MediaStore.MediaColumns.RELATIVE_PATH}=? AND ${MediaStore.MediaColumns.DISPLAY_NAME}=?"
        val args = arrayOf(relativeDir, displayName)
        resolver.query(
            MediaStore.Downloads.EXTERNAL_CONTENT_URI,
            projection,
            selection,
            args,
            null,
        )?.use { c ->
            if (!c.moveToFirst()) return null
            val id = c.getLong(0)
            return ContentUris.withAppendedId(MediaStore.Downloads.EXTERNAL_CONTENT_URI, id)
        }
        return null
    }
}
