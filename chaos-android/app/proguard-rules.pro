# Keep MPV wrapper classes (native entry points).
-keep class dev.jdtech.mpv.** { *; }

# Keep JNA bindings.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.Library { *; }
