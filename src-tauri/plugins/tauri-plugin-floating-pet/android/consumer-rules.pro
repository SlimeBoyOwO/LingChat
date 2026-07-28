# Add project specific ProGuard rules here.
# By default, the flags in this file are appended to flags specified
# in proguard-android-optimize.txt
# You can edit the include path and order by changing the proguardFiles
# directive in build.gradle.kts.

# Keep Tauri plugin classes (they are referenced via reflection).
-keep class app.tauri.** { *; }
-keep class com.noiq.lingchat.floatingpet.** { *; }
