pluginManagement {
    val flutterSdkPath =
        run {
            val properties = java.util.Properties()
            val localProps = file("local.properties")
            if (localProps.exists()) {
                localProps.inputStream().use { properties.load(it) }
            }

            // 优先 local.properties，其次尝试环境变量（方便在不同机器/WSL 同步时不被 Linux 路径卡死）。
            val fromProps = properties.getProperty("flutter.sdk")?.trim()?.takeIf { it.isNotEmpty() }
            val fromEnv = (System.getenv("FLUTTER_ROOT") ?: System.getenv("FLUTTER_HOME"))?.trim()?.takeIf { it.isNotEmpty() }

            // 如果 local.properties 里是 Linux 路径（例如 /home/...），在 Windows 上会被当成相对路径导致 includeBuild 失败。
            val picked = fromProps ?: fromEnv
            require(picked != null) {
                "找不到 Flutter SDK 路径。请在 android/local.properties 设置 flutter.sdk，或设置环境变量 FLUTTER_ROOT。"
            }

            val dir = file(picked)
            require(dir.exists()) {
                "Flutter SDK 路径不存在：$picked\n" +
                    "请检查 android/local.properties 的 flutter.sdk 是否是当前机器的真实路径（Windows 示例：C:\\\\src\\\\flutter），" +
                    "或设置环境变量 FLUTTER_ROOT 指向 Flutter SDK。"
            }

            dir.absolutePath
        }

    includeBuild("$flutterSdkPath/packages/flutter_tools/gradle")

    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

plugins {
    id("dev.flutter.flutter-plugin-loader") version "1.0.0"
    id("com.android.application") version "8.11.1" apply false
    id("org.jetbrains.kotlin.android") version "2.2.20" apply false
}

include(":app")
