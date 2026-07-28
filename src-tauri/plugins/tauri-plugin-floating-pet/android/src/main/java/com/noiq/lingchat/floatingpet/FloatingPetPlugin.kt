package com.noiq.lingchat.floatingpet

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.view.WindowManager
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.ViewCompat
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

    override fun load(webView: WebView) {
        instance = this
        hookKeyboardInsets()
        Log.i(TAG, "FloatingPetPlugin loaded")
    }

    @Volatile private var lastImeVisible: Boolean = false
    @Volatile private var hiddenByKeyboard: Boolean = false

    /**
     * 监听主 Activity 窗口的 IME insets：弹起时临时隐藏悬浮窗，收起时若曾因键盘隐藏则恢复。
     * 阶段 1 只覆盖主 App 内的键盘事件；其他 App 弹起的键盘由系统上叠层被挡实现。
     */
    private fun hookKeyboardInsets() {
        val root = activity.window.decorView
        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            if (imeVisible != lastImeVisible) {
                lastImeVisible = imeVisible
                try {
                    if (imeVisible) {
                        hiddenByKeyboard = true
                        activity.startService(FloatingPetService.hideIntent(activity))
                    } else if (hiddenByKeyboard) {
                        hiddenByKeyboard = false
                        val lastScale = SharedPrefs.create(activity).lastScale
                        val intent = FloatingPetService.showIntent(activity, lastScale)
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                            activity.startForegroundService(intent)
                        } else {
                            activity.startService(intent)
                        }
                    }
                } catch (t: Throwable) {
                    Log.w(TAG, "keyboard insets dispatch failed: ${t.message}")
                }
            }
            insets
        }
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
    fun stopFloatingPetServiceWithConfirmation(invoke: Invoke) {
        activity.runOnUiThread {
            AlertDialog.Builder(activity)
                .setTitle("停止悬浮桌宠")
                .setMessage("确定要停止悬浮桌宠服务吗？之后需要手动重新启动桌宠。")
                .setNegativeButton("取消") { dialog, _ ->
                    dialog.dismiss()
                    invoke.resolve(JSObject().apply { put("stopped", false) })
                }
                .setPositiveButton("停止") { _, _ ->
                    runCatching {
                        activity.startService(FloatingPetService.stopIntent(activity))
                    }.onSuccess {
                        invoke.resolve(JSObject().apply { put("stopped", true) })
                    }.onFailure { error ->
                        invoke.reject(error.message ?: "stopFloatingPetService failed")
                    }
                }
                .setOnCancelListener {
                    invoke.resolve(JSObject().apply { put("stopped", false) })
                }
                .show()
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
                p.triggerObject(EVENT_NAME, event.toJson())
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
