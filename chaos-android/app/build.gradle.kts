// 1. 添加此 Import (通常在文件最顶部)
import com.android.build.api.dsl.ApplicationExtension
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

configure<ApplicationExtension> {
    namespace = "com.zerodevi1.chaos_seed"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.zerodevi1.chaos_seed"
        minSdk = 26
        targetSdk = 36
        versionCode = 2
        versionName = "0.1.1"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Keep APK size + native surface area aligned with our supported devices.
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

//    kotlinOptions {
//        jvmTarget = JavaVersion.VERSION_17.toString()
//    }

    buildFeatures {
        compose = true
    }

    // Keep unit tests JVM-only (no Robolectric). The Android Gradle Plugin provides a mockable android.jar.

    // Kotlin 2.x uses the Compose compiler plugin (applied above).
    packaging {
        jniLibs {
            // libmpv AAR bundles libc++_shared.so; other native deps may also bring it.
            pickFirsts += "**/libc++_shared.so"
            // JNA ships libjnidispatch.so inside an AAR; keep the first if multiple copies appear.
            pickFirsts += "**/libjnidispatch.so"
        }
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

// 4. 添加顶层 kotlin 配置块 (替代原有的 kotlinOptions)
kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    // Compose / Material 3
    implementation(platform("androidx.compose:compose-bom:2026.02.00"))
    implementation("androidx.activity:activity-compose:1.12.4")
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    // Used for pull-to-refresh (pullRefresh/PullRefreshIndicator).
    implementation("androidx.compose.material:material")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.navigation:navigation-compose:2.9.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.10.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.10.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.10.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.10.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")

    // DataStore
    implementation("androidx.datastore:datastore-preferences:1.2.0")

    // Networking / Images
    implementation("com.squareup.okhttp3:okhttp:5.3.2")
    implementation("io.coil-kt:coil-compose:2.7.0")

    // JSON
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.10.0")

    // FFI
    // Use the Android AAR so lib/<abi>/libjnidispatch.so is packaged into the APK.
    implementation("net.java.dev.jna:jna:5.18.1@aar")
    // jna-platform is not used in this project; keep deps minimal to avoid pulling the JAR variant of JNA.

    // Media3 (Exo)
    implementation("androidx.media3:media3-exoplayer:1.9.2")
    implementation("androidx.media3:media3-exoplayer-hls:1.9.2")
    implementation("androidx.media3:media3-ui:1.9.2")

    // MPV (libmpv wrapper + bundled native libs)
    implementation("dev.jdtech.mpv:libmpv:0.5.1") {
        // 告诉 Gradle：引用 mpv 时，不要把它的 JNA 带进来！
        exclude(group = "net.java.dev.jna")
    }

    // Tests
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.2")
}

//configurations.all {
//    resolutionStrategy {
//        // 强制所有模块使用 5.18.1，防止传递依赖引入旧版本
//        force("net.java.dev.jna:jna:5.18.1")
//    }
//}