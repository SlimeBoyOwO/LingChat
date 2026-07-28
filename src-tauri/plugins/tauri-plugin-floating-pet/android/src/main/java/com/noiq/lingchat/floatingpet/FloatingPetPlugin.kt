package com.noiq.lingchat.floatingpet

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.view.WindowManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import org.json.JSONObject

private const val TAG = "FloatingPetPlugin"

private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

/**
 * Tauri 2 Android 桥接插件。
 *
 * 启动时设置静态 instance，供 FloatingPetService 通过 emitEvent 把
 * 手势事件推送回 WebView。WebView 端 listen("floating-pet://event") 接收。
 *
 * 每个 @Command 对应 Rust 端同名 Tauri command 的 JNI 实现。
 */
@TauriPlugin
class FloatingPetPlugin(private val activity: Activity) : Plugin(activity) {

    override fun load() {
        instance = this
        Log.i(TAG, "FloatingPetPlugin loaded")
    }

    @Command
    fun checkOverlayPermission(invoke: Invoke) {
        try {
            val granted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                Settings.canDrawOverlays(activity)
            } else {
                true // pre-M 默认拥有该权限
            }
            val res = JSObject().apply {
                put("granted", granted)
            }
            invoke.resolve(res)
        } catch (e: Exception) {
            invoke.reject(e.message ?: "checkOverlayPermission failed")
        }
    }

    @Command
    fun requestOverlayPermission(invoke: Invoke) {
        try {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
                invoke.resolve(JSObject())
                return
            }
            val intent = Intent(
                Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                Uri.parse("package:${activity.packageName}"),
            ).apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
            activity.startActivity(intent)
            invoke.resolve(JSObject())
        } catch (e: Exception) {
            invoke.reject(e.message ?: "requestOverlayPermission failed")
        }
    }

    @Command
    fun showFloatingPet(invoke: Invoke) {
        @InvokeArg
        class Args {
            var scale: Double? = null
        }
        scope.launch {
            try {
                val args = invoke.parseArgs(Args::class.java)
                val intent = FloatingPetService.showIntent(activity, args.scale?.toFloat())
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    activity.startForegroundService(intent)
                } else {
                    activity.startService(intent)
                }
                invoke.resolve(JSObject())
            } catch (e: Exception) {
                invoke.reject(e.message ?: "showFloatingPet failed")
            }
        }
    }

    @Command
    fun hideFloatingPet(invoke: Invoke) {
        scope.launch {
            try {
                activity.startService(FloatingPetService.hideIntent(activity))
                invoke.resolve(JSObject())
            } catch (e: Exception) {
                invoke.reject(e.message ?: "hideFloatingPet failed")
            }
        }
    }

    @Command
    fun stopFloatingPetService(invoke: Invoke) {
        scope.launch {
            try {
                activity.startService(FloatingPetService.stopIntent(activity))
                invoke.resolve(JSObject())
            } catch (e: Exception) {
                invoke.reject(e.message ?: "stopFloatingPetService failed")
            }
        }
    }

    @Command
    fun updatePetState(invoke: Invoke) {
        @InvokeArg
        class Args {
            var payload: JSONObject? = null
        }
        scope.launch {
            try {
                val args = invoke.parseArgs(Args::class.java)
                val payload = args.payload ?: JSONObject()
                val intent = FloatingPetService.updateIntent(activity, payload.toString())
                activity.startService(intent)
                invoke.resolve(JSObject())
            } catch (e: Exception) {
                invoke.reject(e.message ?: "updatePetState failed")
            }
        }
    }

    @Command
    fun startPermissionExplanation(invoke: Invoke) {
        try {
            if (SharedPrefs.create(activity).permissionExplanationShown) {
                invoke.resolve(JSObject())
                return
            }
            // 必须在主线程构建对话框
            activity.runOnUiThread {
                AlertDialog.Builder(activity)
                    .setTitle("启用悬浮桌宠")
                    .setMessage(
                        "桌宠需要显示在其它应用之上。\n" +
                            "接下来的页面请开启「显示在其它应用上层」权限。\n\n" +
                            "权限仅用于渲染桌宠本身，不会读取其它应用内容。",
                    )
                    .setPositiveButton("去开启") { _, _ ->
                        requestOverlayPermission(invoke)
                    }
                    .setNegativeButton("稍后") { d, _ -> d.dismiss() }
                    .setCancelable(true)
                    .show()
            }
            invoke.resolve(JSObject())
        } catch (e: Exception) {
            invoke.reject(e.message ?: "startPermissionExplanation failed")
        }
    }

    @Command
    fun markPermissionExplanationShown(invoke: Invoke) {
        try {
            SharedPrefs.create(activity).permissionExplanationShown = true
            invoke.resolve(JSObject())
        } catch (e: Exception) {
            invoke.reject(e.message ?: "markPermissionExplanationShown failed")
        }
    }

    companion object {
        @Volatile
        private var instance: FloatingPetPlugin? = null

        /**
         * 由 FloatingPetService -> FloatingPetWindowController -> 事件回调触发，
         * 通过 Tauri 事件总线推送到 WebView。
         */
        fun emitEvent(event: PetEvent) {
            val p = instance ?: return
            try {
                p.trigger(EVENT_NAME, event.toJson())
            } catch (t: Throwable) {
                Log.w(TAG, "emitEvent failed: ${t.message}")
            }
        }

        private const val EVENT_NAME = "floating-pet://event"

        /**
         * 部分 API level 上 WindowManager.LayoutParams 不允许使用 FLAG_NOT_TOUCH_MODAL
         * 与其它标志的组合；这里集中屏蔽 lint 告警。
         */
        @Suppress("unused")
        private val layoutFlags: Int =
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
    }
}
