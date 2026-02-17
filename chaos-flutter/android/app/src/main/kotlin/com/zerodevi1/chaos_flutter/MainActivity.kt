package com.zerodevi1.chaos_flutter

import android.app.PictureInPictureParams
import android.os.Build
import android.os.Environment
import android.util.Rational
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

class MainActivity : FlutterActivity() {
    private val channelName = "chaos_seed/android"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)

        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, channelName)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "getPublicDownloadsDir" -> {
                        // NOTE: 这只是路径字符串；真正能否写入取决于 Android 版本/权限/Scoped Storage。
                        val dir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
                        result.success(dir?.absolutePath)
                    }
                    "exportIntoDownloads" -> {
                        val outDir = (call.argument<String>("outDir") ?: "").trim()
                        val sourcePath = (call.argument<String>("sourcePath") ?: "").trim()
                        val overwrite = call.argument<Boolean>("overwrite") ?: false
                        if (outDir.isEmpty() || sourcePath.isEmpty()) {
                            result.error("invalid_args", "outDir/sourcePath is empty", null)
                            return@setMethodCallHandler
                        }
                        try {
                            val exported = PublicDownloadsExporter.exportIntoChaosSeedDownloads(
                                applicationContext,
                                File(outDir),
                                File(sourcePath),
                                overwrite = overwrite,
                            )
                            result.success(
                                mapOf(
                                    "displayPath" to exported.displayPath,
                                    "skipped" to exported.skipped,
                                ),
                            )
                        } catch (e: Exception) {
                            result.error("export_failed", e.message, null)
                        }
                    }
                    "isPipSupported" -> {
                        val supported = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                        result.success(supported)
                    }
                    "enterPip" -> {
                        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
                            result.success(null)
                            return@setMethodCallHandler
                        }
                        val aspectW = (call.argument<Int>("aspectW") ?: 16).coerceAtLeast(1)
                        val aspectH = (call.argument<Int>("aspectH") ?: 9).coerceAtLeast(1)

                        val builder = PictureInPictureParams.Builder()
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                            builder.setAspectRatio(Rational(aspectW, aspectH))
                        }
                        enterPictureInPictureMode(builder.build())
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
    }
}
