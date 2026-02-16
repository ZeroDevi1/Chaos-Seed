package com.zerodevi1.chaos_flutter

import android.app.PictureInPictureParams
import android.os.Build
import android.os.Environment
import android.util.Rational
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

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
