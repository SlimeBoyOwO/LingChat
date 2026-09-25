package com.noiq.floatingpet

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.webkit.WebView
import android.webkit.WebViewClient
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val TAG = "FloatingPet"

/**
 * 悬浮窗内容 URL。
 *
 * 走 Tauri 的 asset 协议，指向 App 自身的 /pet 路由。
 * 注意：悬浮窗里是**独立的 WebView 实例**，不在 Tauri 的 IPC 上下文里，
 * 因此它只能渲染页面 / 播放语音，无法直接 invoke Tauri 命令。
 * 与主界面的通信走 Plugin.trigger() 事件（见 notifyMain）。
 */
private const val DEFAULT_PET_URL = "http://tauri.localhost/pet"

/** 悬浮窗最小/最大尺寸限制（dp），防止前端传入异常值导致窗口不可见。 */
private const val MIN_SIZE_DP = 80.0
private const val MAX_SIZE_DP = 720.0

// ─── 参数结构（字段名需与 Rust 侧 serde camelCase 对应） ──────────

@InvokeArg
class ShowArgs {
    var url: String = DEFAULT_PET_URL
    var width: Double = 240.0
    var height: Double = 360.0
    var x: Double = 0.0
    var y: Double = 0.0
}

@InvokeArg
class MoveArgs {
    var x: Double = 0.0
    var y: Double = 0.0
}

@InvokeArg
class SizeArgs {
    var width: Double = 240.0
    var height: Double = 360.0
}

@InvokeArg
class TouchableArgs {
    var touchable: Boolean = true
}

/**
 * Android 系统级悬浮窗插件。
 *
 * 使用 `TYPE_APPLICATION_OVERLAY`（API 26+）或 `TYPE_PHONE`（API 26 以下）
 * 创建可浮在其他 App 之上的透明 WebView 窗口。
 *
 * 相比桌面端 `api::pet` 的实现，这里有两个平台差异必须注意：
 *
 * 1. **点击穿透是窗口级开关**：Android 的 `FLAG_NOT_TOUCHABLE` 作用于整个窗口，
 *    无法像 Windows 那样按像素区域判定。因此悬浮窗尺寸应紧贴角色，
 *    不要留大块透明区域，否则会挡住下层 App 的触摸。
 *
 * 2. **权限只能手动授予**：`SYSTEM_ALERT_WINDOW` 是特殊权限，
 *    必须跳转设置页由用户手动开启，无法运行时弹窗申请。
 */
@TauriPlugin
class FloatingPetPlugin(private val activity: Activity) : Plugin(activity) {

    private var windowManager: WindowManager? = null
    private var petView: View? = null
    private var layoutParams: WindowManager.LayoutParams? = null

    /** 触摸可交互状态，与 Rust 侧 FloatingPetState 保持同步。 */
    private var touchable: Boolean = true

    private val density: Float
        get() = activity.resources.displayMetrics.density

    private fun dp(value: Double): Int = (value * density).toInt()

    // ─── 权限 ────────────────────────────────────────────────

    /** 悬浮窗权限是否已授予。API 23 以下默认有权限。 */
    private fun hasOverlayPermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            Settings.canDrawOverlays(activity)
        } else {
            true
        }
    }

    @Command
    fun checkPermission(invoke: Invoke) {
        // resolve(Boolean) 无对应重载，Boolean 走 resolveObject 序列化
        invoke.resolveObject(hasOverlayPermission())
    }

    /**
     * 跳转到系统的悬浮窗授权页。
     *
     * 用户在设置页授权后不会收到回调，前端需在 App 恢复前台时
     * 重新调用 checkPermission 确认。
     */
    @Command
    fun requestPermission(invoke: Invoke) {
        if (hasOverlayPermission()) {
            invoke.resolve()
            return
        }

        try {
            val intent = Intent(
                Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                Uri.parse("package:${activity.packageName}")
            )
            activity.startActivity(intent)
            // 这里只表示「已发起跳转」，不代表用户已授权
            invoke.resolve()
        } catch (e: Exception) {
            Log.e(TAG, "无法打开悬浮窗授权页", e)
            invoke.reject("无法打开授权页: ${e.message}")
        }
    }

    // ─── 显示 / 隐藏 ──────────────────────────────────────────

    @SuppressLint("SetJavaScriptEnabled")
    @Command
    fun show(invoke: Invoke) {
        val args = invoke.parseArgs(ShowArgs::class.java)

        if (!hasOverlayPermission()) {
            invoke.reject("PERMISSION_DENIED")
            return
        }

        activity.runOnUiThread {
            try {
                // 幂等：先清理可能残留的旧窗口，避免 addView 重复导致泄漏
                removePetView()

                val width = dp(args.width.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                val height = dp(args.height.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))

                val webView = WebView(activity).apply {
                    // 关键：透明背景，否则会显示 WebView 默认白底
                    setBackgroundColor(Color.TRANSPARENT)
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    settings.mediaPlaybackRequiresUserGesture = false
                    settings.allowFileAccess = true
                    settings.allowContentAccess = true
                    webViewClient = WebViewClient()
                    loadUrl(args.url.ifBlank { DEFAULT_PET_URL })
                }

                val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
                } else {
                    @Suppress("DEPRECATION")
                    WindowManager.LayoutParams.TYPE_PHONE
                }

                val params = WindowManager.LayoutParams(
                    width,
                    height,
                    type,
                    // FLAG_NOT_FOCUSABLE 让悬浮窗不抢输入焦点（不弹键盘、不挡返回键）
                    // FLAG_LAYOUT_NO_LIMITS 允许贴边/部分出屏
                    WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                        WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
                        WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED,
                    PixelFormat.TRANSLUCENT
                ).apply {
                    gravity = Gravity.TOP or Gravity.START
                    x = dp(args.x)
                    y = dp(args.y)
                }

                applyTouchableFlag(params, touchable)

                val wm = activity.getSystemService(Context.WINDOW_SERVICE) as WindowManager
                wm.addView(webView, params)

                windowManager = wm
                petView = webView
                layoutParams = params

                Log.i(TAG, "悬浮窗已显示 ${width}x${height} @ (${params.x},${params.y})")
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "显示悬浮窗失败", e)
                removePetView()
                invoke.reject("显示悬浮窗失败: ${e.message}")
            }
        }
    }

    @Command
    fun hide(invoke: Invoke) {
        activity.runOnUiThread {
            try {
                removePetView()
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "隐藏悬浮窗失败", e)
                invoke.reject("隐藏悬浮窗失败: ${e.message}")
            }
        }
    }

    /** 必须在主线程调用。 */
    private fun removePetView() {
        val view = petView ?: return
        try {
            windowManager?.removeView(view)
        } catch (e: Exception) {
            // 窗口可能已因 Activity 销毁被系统移除，忽略
            Log.w(TAG, "移除悬浮窗视图时出错（可忽略）", e)
        } finally {
            (view as? WebView)?.let {
                it.loadUrl("about:blank")
                it.destroy()
            }
            petView = null
            layoutParams = null
        }
    }

    // ─── 位置 / 尺寸 / 穿透 ───────────────────────────────────

    @Command
    fun movePet(invoke: Invoke) {
        val args = invoke.parseArgs(MoveArgs::class.java)
        activity.runOnUiThread {
            val params = layoutParams
            val view = petView
            if (params == null || view == null) {
                invoke.reject("NOT_VISIBLE")
                return@runOnUiThread
            }
            try {
                params.x = dp(args.x)
                params.y = dp(args.y)
                windowManager?.updateViewLayout(view, params)
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "移动悬浮窗失败", e)
                invoke.reject("移动悬浮窗失败: ${e.message}")
            }
        }
    }

    @Command
    fun setSize(invoke: Invoke) {
        val args = invoke.parseArgs(SizeArgs::class.java)
        activity.runOnUiThread {
            val params = layoutParams
            val view = petView
            if (params == null || view == null) {
                invoke.reject("NOT_VISIBLE")
                return@runOnUiThread
            }
            try {
                params.width = dp(args.width.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                params.height = dp(args.height.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                windowManager?.updateViewLayout(view, params)
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "调整悬浮窗尺寸失败", e)
                invoke.reject("调整悬浮窗尺寸失败: ${e.message}")
            }
        }
    }

    /**
     * 切换点击穿透。
     *
     * 关闭穿透（touchable=true）时把焦点让给宿主 App，
     * 这样悬浮窗内的输入框才能正常弹出软键盘。
     */
    @Command
    fun setTouchable(invoke: Invoke) {
        val args = invoke.parseArgs(TouchableArgs::class.java)
        activity.runOnUiThread {
            val params = layoutParams
            val view = petView
            if (params == null || view == null) {
                invoke.reject("NOT_VISIBLE")
                return@runOnUiThread
            }
            try {
                touchable = args.touchable
                applyTouchableFlag(params, touchable)
                windowManager?.updateViewLayout(view, params)
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "切换穿透状态失败", e)
                invoke.reject("切换穿透状态失败: ${e.message}")
            }
        }
    }

    /**
     * 按触摸状态调整窗口 Flag。
     *
     * - 需要交互：FLAG_NOT_FOCUSABLE，可触摸但不抢焦点
     * - 需要穿透：额外加 FLAG_NOT_TOUCHABLE，触摸落到下层 App
     *
     * 注意不能移除 FLAG_NOT_FOCUSABLE，否则悬浮窗会抢走返回键和输入焦点。
     */
    private fun applyTouchableFlag(params: WindowManager.LayoutParams, touchable: Boolean) {
        params.flags = if (touchable) {
            params.flags and WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE.inv()
        } else {
            params.flags or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        }
    }

    // ─── 事件通道 ─────────────────────────────────────────────

    /**
     * 向主 WebView 广播事件。
     *
     * 悬浮窗内的 WebView 不在 Tauri IPC 上下文中，无法直接 invoke 命令，
     * 因此需要主界面监听这些事件来接收桌宠的操作（如点击角色、请求设置等）。
     */
    @Suppress("unused")
    private fun notifyMain(event: String, payload: JSObject = JSObject()) {
        try {
            trigger(event, payload)
        } catch (e: Exception) {
            Log.e(TAG, "广播事件失败: $event", e)
        }
    }

    // ─── 生命周期 ─────────────────────────────────────────────

    override fun onDestroy() {
        activity.runOnUiThread { removePetView() }
        super.onDestroy()
    }
}
