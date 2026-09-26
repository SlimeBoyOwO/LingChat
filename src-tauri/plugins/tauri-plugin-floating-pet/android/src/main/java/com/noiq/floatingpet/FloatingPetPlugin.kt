package com.noiq.floatingpet

import android.annotation.SuppressLint
import android.app.Activity
import android.app.Application
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val TAG = "FloatingPet"

/**
 * 悬浮窗内页面的**逻辑尺寸**（dp）。
 *
 * 必须与前端 `src/components/pet/constants.ts` 的 `PET_WIDTH_BASE`（240）、
 * `AVATAR_BAND_BASE`（210）、`CHAT_BASE_H`（70）保持一致。
 *
 * ## 为什么是「固定逻辑尺寸 + 整体缩放」
 *
 * 悬浮窗里的页面**不再**按窗口宽度做响应式布局，而是始终按桌面端那套
 * 240dp 宽的布局排版，再由前端 `transform: scale(window.innerWidth / 240)`
 * 整体等比缩放到窗口大小。
 *
 * 这样做的理由：
 * - 布局只有一套（与桌面端完全相同），不会在小窗里错位、溢出、点不到
 * - 文字/按钮/输入框随窗口等比缩放，小窗下不会「挤成一团」
 * - 页面内容恰好铺满逻辑画布 → 窗口里没有大块透明区域
 *
 * 代价：文字绝对大小与窗口宽度成正比，因此展开态不能太窄——
 * 2/5 屏宽时缩放系数只有 0.6，15px 字缩到 9px 就看不清了。
 */
private const val PET_LOGICAL_WIDTH = 240.0
private const val COLLAPSED_LOGICAL_HEIGHT = 210.0
private const val EXPANDED_LOGICAL_HEIGHT = 280.0

/**
 * 头像态宽度 = 屏幕宽度 × 该比例（用户指定「约 1/6 屏宽」）。
 * 展开态宽度 = 屏幕宽度 × 该比例。
 *
 * 用屏幕比例而非固定 dp，是为了让桌宠在不同尺寸/DPI 的机器上
 * 视觉占比一致——固定 dp 在小屏上会显得过大（这正是此前 240dp
 * 占了普通手机 60% 屏宽的原因）。
 *
 * 展开态取 0.6 而不是 2/5：逻辑宽 240dp 缩到 0.6×360=216dp 时
 * 缩放系数 0.9，15px 的字约 13.5px，勉强可读；2/5 屏宽（144dp）
 * 只有 0.6 倍，字会小到看不清。
 */
private const val COLLAPSED_WIDTH_RATIO = 1.0 / 6.0
private const val EXPANDED_WIDTH_RATIO = 0.6

/**
 * 窗口高度与宽度之比 —— 直接由逻辑尺寸推出，保证原生给的初始尺寸
 * 与前端按同一套常量算出的内容高度一致，避免「先给一个错的高度、
 * 前端再纠正一次」造成的闪动。
 *
 * 收起态只有头像（210）；展开态是头像 + 输入框（210 + 70 = 280）。
 * 气泡出现时窗口高度由前端通过 `set_size` 再撑高，不在这里预留。
 */
private const val COLLAPSED_HEIGHT_RATIO = COLLAPSED_LOGICAL_HEIGHT / PET_LOGICAL_WIDTH
private const val EXPANDED_HEIGHT_RATIO = EXPANDED_LOGICAL_HEIGHT / PET_LOGICAL_WIDTH

/**
 * 尺寸兜底上下限（dp），防止异常比例算出不可见或超屏的窗口。
 *
 * 下限必须够小：收起态窗口只有约 1/6 屏宽（360dp 屏上约 60dp），
 * 高度约 52dp。此前下限 80dp 会把前端报上来的高度硬抬到 80，
 * 于是收起态窗口下方多出一块透明区域。
 */
private const val MIN_SIZE_DP = 24.0
private const val MAX_SIZE_DP = 720.0

/**
 * 桌宠 WebView 保活轮询间隔（毫秒）。
 *
 * 见 [FloatingPetPlugin.startKeepAlive]：宿主 Activity 一旦 onPause，
 * wry 会调 `mWebView.onPause()` 把桌宠冻住，而重入时机在不同 ROM /
 * Tauri 版本上并不稳定，因此除了生命周期回调里补一次，还用一个
 * 低频轮询兜底。
 */
private const val KEEP_ALIVE_INTERVAL_MS = 500L

/**
 * 收回时重发 `pet-attached` 的延迟序列（毫秒）。
 *
 * 用户常常是在**别的 App 里**触发收回的，此时宿主 Activity 还处于停止态，
 * WebView 刚被唤醒、窗口也还没重新布局，`evaluateJavascript` 会被丢掉。
 * 这条事件一旦丢失，页面就一直停在悬浮窗布局（表现为「切回去只有左上一角、
 * 也不回聊天页」）。
 *
 * 因此这里既拉长重试跨度，又在 Activity 真正回到前台时补发一次
 * （见 [ensureLifecycleCallbacks] 的 `onActivityResumed`）——单纯靠固定
 * 延迟重试是不够的，因为「用户什么时候切回来」完全不可预测。
 */
private val PET_ATTACHED_RETRY_DELAYS_MS = longArrayOf(0L, 300L, 1000L, 3000L)

/**
 * 收回后保活轮询继续运行的时长（毫秒）。
 *
 * 不能一收回就 [FloatingPetPlugin.stopKeepAlive]：WebView 从 60dp 的
 * 悬浮窗被塞回整屏 Activity 时，Chromium 的视口要重新算一次，而宿主
 * Activity 此刻往往还在后台、不跑布局遍历。轮询多撑一会儿，等用户切回来、
 * 视口真正更新完再停，否则整个 App 会以悬浮窗的窄视口渲染
 * ——看起来就是「只有左上一角」。
 */
private const val KEEP_ALIVE_GRACE_MS = 20000L

/**
 * `hide` 命令延迟执行的时间（毫秒）。
 *
 * 收回是页面点「返回」触发的，而那次点击此刻正由 WebView 分发。
 * 若在同一个消息循环里同步 `removeViewImmediate` + `setContentView`，
 * 等于在输入分发途中把 WebView 从窗口上摘下来，输入通道与 ViewRootImpl
 * 会互相等待 → 界面卡死（与早期「双击收回卡死」是同一个坑）。
 * 让出一拍，等触摸分发跑完再搬。
 */
private const val HIDE_DELAY_MS = 80L

/**
 * 单击与拖拽的判定阈值（dp）：按下到抬起位移超过它就算拖动，不触发点击。
 *
 * 取 16dp 而不是 Android 默认的 8dp。这里判定的是「整个窗口要不要跟着
 * 手指走」，不是滚动，因此容差该给得比 `ViewConfiguration` 的 touchSlop
 * 宽：手指点按时天然会带几 dp 位移，8dp 会把大量正常点按判成拖动
 * ——表现就是「单击经常没反应，窗口还会被带偏一点」。
 */
private const val TAP_SLOP_DP = 16.0


// ─── 参数结构（字段名需与 Rust 侧 serde camelCase 对应） ──────────

@InvokeArg
class ShowArgs {
    var scale: Double = 1.0
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

@InvokeArg
class ExpandedArgs {
    var expanded: Boolean = false
}

/**
 * Android 系统级悬浮窗插件 —— **搬运主 WebView** 的实现。
 *
 * ## 为什么是「搬运」而不是「新建」
 *
 * 桌面端的桌宠**不是新窗口**：它把 `label="main"` 那个窗口改属性
 * （`set_decorations(false)` + `set_size` + `set_always_on_top`），
 * 于是同一个 WebView 从「聊天界面」变成了「桌宠」——IPC、store、
 * 路由状态全部原样保留。
 *
 * 早期版本在悬浮窗里 `WebView(activity)` 新建了一个实例，结果是
 * 一个**没有 IPC 的空壳**：读不到角色数据、发不出消息，只能渲染静态页面。
 *
 * 现在改为搬运主 WebView 本身：
 *
 * 1. wry 用 `activity.setContentView(webView)` 把 Tauri 的 WebView
 *    设成 Activity 的**根内容视图**（见 wry `android/main_pipe.rs`）。
 * 2. Tauri 的 IPC 是 `webView.addJavascriptInterface(ipc, "ipc")`，
 *    **绑定在 WebView 对象上，不绑定在窗口上**。
 * 3. 因此把同一个 View 从 Activity 视图树移到 `WindowManager`，
 *    JS 上下文不重载、`invoke()` 照常可用、store 数据完整。
 *
 * ## 占位视图
 *
 * WebView 是 Activity 的唯一内容视图，搬走后 Activity 就空了（白屏）。
 * 因此在搬走前先 `setContentView(占位页)`，用户切回 App 时看到的是
 * 一张引导图，而不是空白。
 *
 * ## 生命周期风险
 *
 * `WryActivity.mWebView` 是 `lateinit`，`onPause/onResume/onDestroy`
 * 都会直接访问它。搬运期间必须避免这些回调把 WebView 销毁掉——
 * 具体做法见 [[petDetached]] 与 [[restoreWebViewToActivity]]。
 */
@TauriPlugin
class FloatingPetPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        /**
         * 当前活跃实例，供注入到 WebView 的 [PetBridge] 回调使用。
         * 同一时刻只应存在一个悬浮窗，因此单引用足够。
         */
        @Volatile
        private var instance: FloatingPetPlugin? = null
    }

    private var windowManager: WindowManager? = null

    /** 搬进悬浮窗的那个 View（就是主 WebView）。 */
    private var petView: View? = null
    private var layoutParams: WindowManager.LayoutParams? = null

    /** 触摸可交互状态，与 Rust 侧 FloatingPetState 保持同步。 */
    private var touchable: Boolean = true

    /** 当前是否处于展开态（头像 + 输入框）。 */
    private var expanded: Boolean = false

    /** 桌宠缩放系数，来自设置。 */
    private var petScale: Double = 1.0

    /**
     * WebView 是否已被搬离 Activity。
     *
     * 用于在 Activity 生命周期回调里判断「WebView 不在视图树上」，
     * 避免把悬浮窗里的 WebView 当成主界面 WebView 处理。
     */
    private var petDetached: Boolean = false

    /**
     * 桌宠 WebView 的保活轮询（见 [startKeepAlive]）。
     *
     * 用主线程 Handler 而不是 Timer：`WebView.onResume()` / `resumeTimers()`
     * 都必须在 UI 线程调用。
     */
    private val keepAliveHandler = Handler(Looper.getMainLooper())
    private var keepAliveRunning = false
    private val keepAliveTick = object : Runnable {
        override fun run() {
            if (!keepAliveRunning) return
            // 收回之后（petDetached=false）也要继续唤醒：WebView 从 60dp 的
            // 悬浮窗被塞回整屏 Activity，Chromium 的视口要重算一次，而宿主
            // Activity 此刻往往还在后台、不跑布局遍历。不继续撑着的话，
            // 整个 App 会以悬浮窗的窄视口渲染 —— 就是「只有左上一角」。
            (petView as? WebView)?.let { resumePetWebView(it, "keep-alive") }
            // 顺带重推一次窗口几何：前端靠它算缩放系数，而 WebView 的视口
            // 在原生改完尺寸后会滞后一会儿，自算必然出错。低频重推让页面
            // 即使漏掉某次事件也能在半秒内自愈。
            if (petDetached) layoutParams?.let { notifyMetrics(it, petView) }
            keepAliveHandler.postDelayed(this, KEEP_ALIVE_INTERVAL_MS)
        }
    }

    /**
     * 收回后仍待补发的 `pet-attached` 目标 WebView。
     *
     * 见 [ensureLifecycleCallbacks]：用户多半是在别的 App 里点 ✕ 收回的，
     * 那时宿主 Activity 处于停止态，`evaluateJavascript` 会被丢弃；
     * 等用户真正切回来（Activity 重新 resume）再补发一次才可靠。
     */
    private var pendingAttachNotify: WebView? = null

    /** [ensureLifecycleCallbacks] 的幂等标记。 */
    private var lifecycleCallbacksRegistered = false

    /**
     * 注册进程级 Activity 生命周期回调。
     *
     * ## 为什么不能用 Tauri 的插件生命周期
     *
     * Tauri 2.11.1 里 `PluginManager.onPause/onResume/onStop` 是**死代码**
     * （`TauriLifecycleObserver` 从未被 `addObserver()` 注册），
     * 因此插件拿不到「用户切回 App」这个时机。改用 Android 自己的
     * `Application.ActivityLifecycleCallbacks`——它由系统直接分发，不受
     * Tauri 版本影响。
     *
     * 目前只用到 `onActivityResumed`：把收回时没送达的 `pet-attached`
     * 补发给页面。这条事件一旦丢失，页面就会一直停在悬浮窗布局。
     */
    private fun ensureLifecycleCallbacks() {
        if (lifecycleCallbacksRegistered) return
        lifecycleCallbacksRegistered = true
        try {
            activity.application.registerActivityLifecycleCallbacks(
                object : Application.ActivityLifecycleCallbacks {
                    override fun onActivityCreated(a: Activity, b: Bundle?) = Unit
                    override fun onActivityStarted(a: Activity) = Unit

                    override fun onActivityResumed(a: Activity) {
                        val wv = pendingAttachNotify ?: return
                        pendingAttachNotify = null
                        Log.i(TAG, "Activity 已回到前台，补发 pet-attached")
                        notifyWeb(wv, "pet-attached", JSObject())
                    }

                    override fun onActivityPaused(a: Activity) = Unit
                    override fun onActivityStopped(a: Activity) = Unit
                    override fun onActivitySaveInstanceState(a: Activity, b: Bundle) = Unit
                    override fun onActivityDestroyed(a: Activity) = Unit
                }
            )
        } catch (e: Exception) {
            Log.w(TAG, "注册 Activity 生命周期回调失败（可忽略）", e)
        }
    }

    /**
     * 开始保活轮询。
     *
     * ## 为什么需要轮询而不是只靠生命周期回调
     *
     * 宿主 Activity 一旦 onPause，`WryActivity.onPause()` 会调用
     * `mWebView.onPause()`，把搬进悬浮窗的**同一个** WebView 冻住——
     * 渲染停、`requestAnimationFrame` 停、JS 定时器停，桌宠在桌面上
     * 静止不动，并且 `evaluateJavascript` 也不再执行（于是原生派发的
     * `pet-attached` 等事件全部丢失）。
     *
     * 直觉上应该在插件的 `onPause` 钩子里补一次 `onResume()`，但**这条路
     * 在 Tauri 2.11.1 上走不通**：`PluginManager.onPause/onResume/onStop`
     * 唯一的上游是 `TauriLifecycleObserver`，而它只被定义、
     * **从未被 `addObserver()` 注册**（见 `mobile/android-codegen/TauriActivity.kt`）。
     * 也就是说插件的那两个覆写目前在真机上根本不会被调用。
     *
     * 因此这里用 500ms 一次的轮询兜底：不依赖任何生命周期回调，只要还在
     * 悬浮窗里就持续把 WebView 拉回运行态。`onResume()` / `resumeTimers()`
     * 都是幂等的，重复调用没有副作用。
     */
    private fun startKeepAlive() {
        if (keepAliveRunning) return
        keepAliveRunning = true
        keepAliveHandler.postDelayed(keepAliveTick, KEEP_ALIVE_INTERVAL_MS)
        Log.i(TAG, "已启动桌宠 WebView 保活轮询")
    }

    /** 停止保活轮询（WebView 已还给 Activity 时调用）。 */
    private fun stopKeepAlive() {
        if (!keepAliveRunning) return
        keepAliveRunning = false
        keepAliveHandler.removeCallbacks(keepAliveTick)
        Log.i(TAG, "已停止桌宠 WebView 保活轮询")
    }

    private val density: Float
        get() = activity.resources.displayMetrics.density

    private fun dp(value: Double): Int = (value * density).toInt()

    /**
     * 屏幕可用宽度（dp）。
     *
     * 用 `resources.displayMetrics.widthPixels` 而不是 `WindowManager.currentWindowMetrics`：
     * 后者在部分 ROM 上返回值受多窗口/折叠屏影响，且 API 30 才有。
     * 这里要的是「这块屏幕多宽」这个稳定物理量。
     */
    private fun screenWidthDp(): Double =
        activity.resources.displayMetrics.widthPixels / density.toDouble()

    private fun screenHeightDp(): Double =
        activity.resources.displayMetrics.heightPixels / density.toDouble()

    /** 收起态尺寸：宽度约 1/6 屏宽。 */
    private fun collapsedSize(): Pair<Int, Int> {
        val w = (screenWidthDp() * COLLAPSED_WIDTH_RATIO * petScale)
            .coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
        val h = (w * COLLAPSED_HEIGHT_RATIO).coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
        return dp(w) to dp(h)
    }

    /** 展开态尺寸：宽度约 2/5 屏宽。 */
    private fun expandedSize(): Pair<Int, Int> {
        val w = (screenWidthDp() * EXPANDED_WIDTH_RATIO * petScale)
            .coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
        val h = (w * EXPANDED_HEIGHT_RATIO).coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
        return dp(w) to dp(h)
    }

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

    /**
     * 查询实时状态与**窗口几何**。
     *
     * ## 为什么前端要主动查，而不是等原生推
     *
     * 原生往页面推事件只能用 `evaluateJavascript`，而这条路在搬运/收回
     * 前后并不可靠（WebView 刚被挂上、宿主 Activity 还在后台、视口尚未
     * 就绪……）。反过来 **页面 → 原生** 的 Tauri IPC 是稳的——用户的点击、
     * 发消息都走它。
     *
     * 因此把「缩放系数」和「我还在不在悬浮窗里」都做成可查询的：
     * 页面轮询这个命令即可自愈，不必赌某一次事件有没有送达。
     *
     * `scale = 窗口宽度(dp) / 240`，与前端 `transform: scale()` 用的是同一个
     * 值。前端**不能**自己从 `window.innerWidth` 推：原生改完窗口尺寸后
     * WebView 视口要过一会儿才跟上，那时读到的宽度是滞后的，算出来的系数
     * 偏小 → 内容只占窗口一角、展开后一大片空白。
     */
    @Command
    fun status(invoke: Invoke) {
        val params = layoutParams
        val view = petView
        val visible = petDetached && view != null
        invoke.resolveObject(
            JSObject().apply {
                put("supported", true)
                put("granted", hasOverlayPermission())
                put("visible", visible)
                put("detached", petDetached)
                // 不在悬浮窗里时给中性值，前端会忽略
                put("scale", if (params != null) currentScale(params, view) else 1.0)
                put(
                    "width",
                    if (params != null) actualWidthPx(params, view) / density.toDouble() else 0.0
                )
                put("height", if (params != null) params.height / density.toDouble() else 0.0)
            }
        )
    }

    // ─── 搬运主 WebView ───────────────────────────────────────

    /**
     * 展示桌宠悬浮窗——把主 WebView 搬进悬浮窗。
     *
     * 调用前提：前端**已经先把页面切到 /pet 路由**。
     * 顺序不能反，否则用户会看到主界面闪一下才变成桌宠。
     *
     * 注意本命令必须在主线程执行：`setContentView` / `addView`
     * 都是 UI 操作，且 WebView 的父容器变更只能在主线程做。
     */
    @Command
    fun show(invoke: Invoke) {
        val args = invoke.parseArgs(ShowArgs::class.java)

        if (!hasOverlayPermission()) {
            invoke.reject("PERMISSION_DENIED")
            return
        }

        activity.runOnUiThread {
            try {
                // 幂等：已经搬过一次时，先把 WebView 完整地还给 Activity
                // 再重新搬。这里必须用 restore 而不是 detach —— detach 只是
                // 断开引用，WebView 会变成无父容器的孤儿，随后
                // findMainWebView() 就再也找不到它了。
                // notifyPage=false：马上又会搬回去，没必要让页面闪一次「已回到 App」。
                if (petView != null) {
                    restoreWebViewToActivity(notifyPage = false)
                }

                petScale = args.scale.coerceIn(0.5, 2.0)

                val webView = findMainWebView()
                if (webView == null) {
                    invoke.reject("主 WebView 尚未创建，无法搬入悬浮窗")
                    return@runOnUiThread
                }

                // ── 1. 先把 WebView 从 Activity 视图树摘下 ──
                // 必须先摘再 setContentView(占位页)：setContentView 是「整个
                // 内容视图替换」，直接调用会让 WebView 随旧视图树一起被移除，
                // 而我们还需要这个实例，因此显式摘下、持有引用。
                //
                // 记到 petView：此后任何一步失败，catch 里的 rollback 都能
                // 通过 restoreWebViewToActivity() 把它装回去。若只在 addView
                // 成功后才赋值，addView 之前的失败会导致 WebView 无家可归
                // → 主界面永久黑屏。
                petView = webView
                petDetached = true

                val root = activity.findViewById<ViewGroup>(android.R.id.content)
                root?.removeView(webView)

                // ── 2. 给 Activity 塞占位页，避免白屏 ──
                activity.setContentView(buildPlaceholderView())

                // ── 3. 把 WebView 放进悬浮窗 ──
                val (width, height) = collapsedSize()
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
                    // FLAG_NOT_FOCUSABLE：默认不抢输入焦点（不弹键盘、不挡返回键）
                    // FLAG_LAYOUT_NO_LIMITS：允许气泡绘制到窗口外/贴边
                    // FLAG_HARDWARE_ACCELERATED：WebView 需要硬件加速渲染
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

                // WebView 的背景必须透明，否则悬浮窗是个白方块。
                // 主界面里它是贴满屏幕的不透明内容，这里改成透明不影响
                // 页面自身的 html/body 背景（由 /pet 路由控制）。
                webView.setBackgroundColor(Color.TRANSPARENT)

                // 拖动手势：挂在整个窗口上；点按一律交回页面
                webView.setOnTouchListener(buildPetTouchListener())

                // 注意：petView / petDetached 已在步骤 1 赋值（为了让 rollback
                // 在任何一步失败时都能把 WebView 装回去），这里只补窗口相关状态。
                windowManager = wm
                layoutParams = params
                expanded = false
                instance = this

                // 通知页面：你现在在悬浮窗里了。
                // 页面据此切换为「仅头像」布局——这必须发生在 addView 之后，
                // 因为 evaluateJavascript 需要有可用的 WebView 实例。
                notifyWeb(webView, "pet-detached", JSObject())

                // 紧接着把权威几何推给页面：收起态的缩放系数约 0.25，
                // 页面必须用它来 scale，否则会按挂载时读到的整屏宽度
                // （系数约 1.5）渲染，头像被裁得只剩一块。
                notifyMetrics(params, webView)

                // 拉起前台服务：桌宠要长期浮在桌面上，必须有前台优先级，
                // 否则 App 退到后台后进程被回收，悬浮窗会直接消失。
                PetForegroundService.start(activity)

                // 并启动 WebView 保活轮询：前台服务只保证**进程**不被回收，
                // 不阻止 wry 在 Activity onPause 时把 WebView 暂停。
                startKeepAlive()

                // 注册 Activity 生命周期回调（幂等）：收回时若 App 在后台，
                // 靠它在用户切回来时补发 pet-attached。
                ensureLifecycleCallbacks()

                Log.i(TAG, "桌宠已展开 ${width}x${height} @ (${params.x},${params.y})")
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "搬入悬浮窗失败", e)
                // 失败时必须把 WebView 还给 Activity，否则主界面永久黑屏
                restoreWebViewToActivity()
                invoke.reject("搬入悬浮窗失败: ${e.message}")
            }
        }
    }

    /**
     * 收起桌宠：把 WebView 搬回 Activity，恢复主界面。
     *
     * 与桌面端 `set_pet_mode(enable=false)` 对应——那边是恢复窗口属性，
     * 这边是恢复视图父子关系，本质都是「同一个 WebView 换个形态」。
     */
    @Command
    fun hide(invoke: Invoke) {
        // 让出一拍再搬：这个命令由页面点「返回」触发，而那次点击此刻正在
        // 由 WebView 分发。同步摘窗口会死锁，见 HIDE_DELAY_MS。
        keepAliveHandler.postDelayed(
            {
                try {
                    restoreWebViewToActivity()
                    invoke.resolve()
                } catch (e: Exception) {
                    Log.e(TAG, "恢复主界面失败", e)
                    invoke.reject("恢复主界面失败: ${e.message}")
                }
            },
            HIDE_DELAY_MS
        )
    }

    /**
     * 把 WebView 从悬浮窗搬回 Activity 内容视图，撤掉占位页。
     *
     * 必须在主线程调用。
     *
     * @param notifyPage 是否向页面派发 `pet-attached`。
     *   `show()` 的幂等分支会先 restore 再重新搬，这种情况下页面马上又
     *   会收到 `pet-detached`，中间那次 attached 只会造成布局闪动，
     *   因此传 false 跳过。
     */
    private fun restoreWebViewToActivity(notifyPage: Boolean = true) {
        val view = petView
        if (view != null) {
            try {
                // 必须用 removeViewImmediate 而不是 removeView：
                // removeView 是**异步**的，它只是把移除动作 post 给
                // ViewRootImpl，返回时 view.parent 仍未清空。
                // 紧接着的 activity.setContentView(view) 会因此抛
                // 「The specified child already has a parent」，
                // 结果是 WebView 既不在悬浮窗、也没进 Activity → 界面卡死。
                // removeViewImmediate 同步摘除，保证下面 setContentView 时
                // view.parent 已经是 null。
                windowManager?.removeViewImmediate(view)
            } catch (e: Exception) {
                // 窗口可能已被系统移除，忽略
                Log.w(TAG, "移除悬浮窗视图时出错（可忽略）", e)
            }
            view.setOnTouchListener(null)
        }

        petView = null
        layoutParams = null
        windowManager = null
        expanded = false
        instance = null

        // 桌宠已收回，不再需要前台优先级
        PetForegroundService.stop(activity)

        if (view != null && petDetached) {
            // 直接 setContentView(webView) 即可完成内容视图替换，无需手动 addView。
            activity.setContentView(view)
            view.setBackgroundColor(Color.TRANSPARENT)

            // 恢复 WebView 的渲染与 JS 定时器：悬浮窗期间可能因宿主 Activity
            // 进入后台而被 WryActivity.onPause() 暂停过（见本类 onResume）。
            // 注意 onResume/resumeTimers 是 WebView 的方法，不是 View 的，
            // 必须转型后再调。
            //
            // 这里必须**先**唤醒再派发事件：WebView 处于暂停态时
            // evaluateJavascript 不会执行，pet-attached 会直接丢失，
            // 页面就永远停在悬浮窗布局里（表现为「收回后只剩一小块」）。
            val wv = view as? WebView
            wv?.let { resumePetWebView(it, "restore") }

            // 通知页面：你已经回到 App 里了，恢复正常布局。
            //
            // 必须 post 到下一轮循环：此刻视图层级刚被重挂，WebView 还在
            // 重新测量/布局，立即 evaluateJavascript 可能落在一个尚未就绪的
            // 渲染上下文里。
            //
            // 光靠固定延迟重试是不够的：用户多半是在**别的 App 里**点 ✕
            // 收回的，宿主 Activity 此刻处于停止态，这几次 evaluateJavascript
            // 会被整体丢弃，而「用户什么时候切回来」完全不可预测。
            // 因此除了这里的重试，还把 wv 记进 pendingAttachNotify，
            // 由 Activity 真正 resume 时补发（见 ensureLifecycleCallbacks）。
            //
            // 注意这里显式把 view 传进去，不能依赖 petView —— 上面已经置空。
            if (notifyPage) {
                pendingAttachNotify = wv
                for (delay in PET_ATTACHED_RETRY_DELAYS_MS) {
                    view.postDelayed({ notifyWeb(wv, "pet-attached", JSObject()) }, delay)
                }
                // 兜底清理：万一 Activity 一直没 resume（例如用户再也没回来），
                // 不要让引用一直挂着。
                keepAliveHandler.postDelayed(
                    { pendingAttachNotify = null },
                    KEEP_ALIVE_GRACE_MS
                )
            }

            // 保活轮询**不能立刻停**：WebView 从 60dp 的悬浮窗被塞回整屏
            // Activity 时，Chromium 的视口要重算一次，而宿主 Activity 此刻
            // 往往还在后台、不跑布局遍历。轮询多撑 20 秒，等用户切回来、
            // 视口真正更新完再停——否则整个 App 会以悬浮窗的窄视口渲染，
            // 看起来就是「切回去只有左上一角」。
            keepAliveHandler.postDelayed(
                {
                    // 期间若又重新进入悬浮窗，就别停了
                    if (!petDetached) stopKeepAlive()
                },
                KEEP_ALIVE_GRACE_MS
            )
        }
        petDetached = false
    }

    /**
     * 断开桌宠视图（插件的 onDestroy 路径）。
     *
     * 与 [restoreWebViewToActivity] 的区别：这里**不**把 WebView 还给
     * Activity —— Activity 本身正在销毁，装回去没有意义，反而可能在
     * 销毁流程里制造新的引用。只把窗口摘掉、断开插件侧的引用即可。
     *
     * 不销毁 WebView 实例：它归 Tauri 管，Tauri 自己的
     * `Rust.onWebviewDestroy` 会负责释放。这里手动 destroy 会造成
     * 二次销毁。
     */
    private fun detachPetView() {
        val view = petView ?: return
        try {
            windowManager?.removeView(view)
        } catch (e: Exception) {
            Log.w(TAG, "移除悬浮窗视图时出错（可忽略）", e)
        }
        view.setOnTouchListener(null)
        petView = null
        layoutParams = null
        windowManager = null
        instance = null
        petDetached = false
        pendingAttachNotify = null
        stopKeepAlive()
        // Activity 正在销毁，前台服务若继续留着会变成没有悬浮窗的空服务
        PetForegroundService.stop(activity)
    }

    /**
     * 找到 Tauri 的主 WebView。
     *
     * 不用 `WryActivity.mWebView`：那是 `private lateinit`，插件拿不到，
     * 且改生成文件会被 `tauri android init` 覆盖。这里从内容视图递归查找，
     * 稳定且不依赖生成代码内部结构。
     */
    private fun findMainWebView(): WebView? {
        val root = activity.findViewById<ViewGroup>(android.R.id.content) ?: return null
        return findWebView(root)
    }

    private fun findWebView(parent: ViewGroup): WebView? {
        for (i in 0 until parent.childCount) {
            when (val child = parent.getChildAt(i)) {
                is WebView -> return child
                is ViewGroup -> findWebView(child)?.let { return it }
            }
        }
        return null
    }

    /**
     * 构造 Activity 的占位页（WebView 被搬走期间显示）。
     *
     * 用户切回 App 时看到的不是白屏，而是一张引导图 ——
     * 提示桌宠正在桌面上运行，点击悬浮窗即可回来。
     *
     * 用纯代码构造 View，不引入 layout 资源文件：插件目录下加资源
     * 需要额外的 gradle 配置，收益不抵成本。
     */
    private fun buildPlaceholderView(): View {
        val context = activity
        val root = android.widget.FrameLayout(context).apply {
            setBackgroundColor(Color.parseColor("#101014"))
        }

        // 背景：App 图标放大、淡化后铺底，保持与 App 一致的视觉调性。
        // 用 applicationInfo.icon —— 它是 App 自己的资源，不需要往插件目录
        // 加 drawable（那要额外的 gradle 配置）。icon 为 0 时 setImageResource
        // 会抛异常，因此整段包在 try 里，失败就只留纯色底。
        try {
            val iconId = activity.applicationInfo.icon
            if (iconId != 0) {
                val bg = android.widget.ImageView(context).apply {
                    setImageResource(iconId)
                    scaleType = android.widget.ImageView.ScaleType.CENTER
                    alpha = 0.12f
                }
                root.addView(
                    bg,
                    android.widget.FrameLayout.LayoutParams(
                        android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
                        android.widget.FrameLayout.LayoutParams.MATCH_PARENT
                    )
                )
            }
        } catch (e: Exception) {
            Log.w(TAG, "占位页背景图加载失败（可忽略）", e)
        }

        // 文案用两行：主句说明现状，次句给出操作。
        // 用户切回 App 看到的应该是「桌宠还在，怎么回去」而不是空白。
        val container = android.widget.LinearLayout(context).apply {
            orientation = android.widget.LinearLayout.VERTICAL
            gravity = Gravity.CENTER
        }

        val title = android.widget.TextView(context).apply {
            text = "桌宠正在桌面上陪着你"
            setTextColor(Color.parseColor("#E6FFFFFF"))
            textSize = 18f
            gravity = Gravity.CENTER
        }
        container.addView(title)

        val hint = android.widget.TextView(context).apply {
            text = "轻点头像展开输入框，再点一下收起；展开后点右上角 ✕ 收回"
            setTextColor(Color.parseColor("#99FFFFFF"))
            textSize = 13f
            gravity = Gravity.CENTER
            setPadding(0, dp(12.0), 0, 0)
        }
        container.addView(hint)

        root.addView(
            container,
            android.widget.FrameLayout.LayoutParams(
                android.widget.FrameLayout.LayoutParams.WRAP_CONTENT,
                android.widget.FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.CENTER
                // 左右留边，避免长文案顶到屏幕边缘
                marginStart = dp(32.0)
                marginEnd = dp(32.0)
            }
        )

        return root
    }

    // ─── 拖动 ─────────────────────────────────────────────────

    /**
     * 悬浮窗的触摸处理：只负责拖动，其余点按一律交回给页面。
     *
     * 手机没有鼠标，桌面端那套 `mouseenter/mouseleave` 展开输入框的逻辑
     * 完全不适用，因此这里只补一件事：
     *
     * - **拖动**：按下到抬起位移超过 [TAP_SLOP_DP] → 移动窗口，松手吸附边缘
     * - **其余点按**：原样交回 WebView，页面据此点头像展开/收起、点按钮
     *
     * ## 为什么不再做「双击收回」
     *
     * 双击和「点头像展开/收起」是**直接冲突**的：同一位置的两次点按，
     * 既可能是「展开 → 收起」，也可能是「收回」，物理上无法区分。
     * 再加上原来的判定只看时间不看位置，任意两次 300ms 内的点按都算双击
     * （点完头像紧接着点输入框也会把桌宠收回去），真机误触严重。
     *
     * 现在收回改由展开面板里的 ✕ 按钮负责（见 `PetMode.vue`），
     * 手势只剩「单击头像 = 展开/收起」一种，没有歧义。
     *
     * 阈值判断仍然必须：没有它，每次拖动都会被页面当成一次点击。
     */
    @SuppressLint("ClickableViewAccessibility")
    private fun buildPetTouchListener(): View.OnTouchListener {
        var downX = 0f
        var downY = 0f
        var startX = 0
        var startY = 0
        var dragging = false
        val slop = dp(TAP_SLOP_DP).toFloat()

        return View.OnTouchListener { v, event ->
            val params = layoutParams ?: return@OnTouchListener false
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    downX = event.rawX
                    downY = event.rawY
                    startX = params.x
                    startY = params.y
                    dragging = false
                    // 交回给 WebView：页面的点头像/点按钮等交互才能工作
                    false
                }

                MotionEvent.ACTION_MOVE -> {
                    val dx = event.rawX - downX
                    val dy = event.rawY - downY
                    if (!dragging && (Math.abs(dx) > slop || Math.abs(dy) > slop)) {
                        dragging = true
                        // 一旦转为拖动，通知 WebView 取消它已经开始的手势。
                        // ACTION_DOWN 是交回给它的，若不给 CANCEL，页面那边会
                        // 一直停在「按下未抬起」的状态（按钮保持按压态、
                        // 输入框可能进入选择模式）。
                        try {
                            val cancel = MotionEvent.obtain(event)
                            cancel.action = MotionEvent.ACTION_CANCEL
                            v.dispatchTouchEvent(cancel)
                            cancel.recycle()
                        } catch (e: Exception) {
                            Log.w(TAG, "取消 WebView 手势失败（可忽略）", e)
                        }
                    }
                    if (dragging) {
                        params.x = startX + dx.toInt()
                        params.y = startY + dy.toInt()
                        try {
                            windowManager?.updateViewLayout(petView, params)
                        } catch (e: Exception) {
                            Log.w(TAG, "拖动悬浮窗失败（可忽略）", e)
                        }
                    }
                    dragging
                }

                MotionEvent.ACTION_UP -> {
                    if (dragging) {
                        // 拖动结束：吸附到屏幕边缘，避免挡住中间内容
                        snapToEdge(params)
                        dragging = false
                        true
                    } else {
                        // 单击：交回给页面（点头像 → 展开/收起）
                        false
                    }
                }

                else -> dragging
            }
        }
    }

    /**
     * 把窗口位置夹回屏幕内。
     *
     * 展开态是**以中心为锚点**放大的，若桌宠原本贴着屏幕下沿，
     * 放大后窗口下半部分会跑到屏幕外——输入框正好在那里，用户就
     * 「看不到也点不到」了。尺寸变化后一律夹一次。
     *
     * 宽度超过屏幕时左对齐（此时 x 已无意义，保证左边缘可见）。
     */
    private fun clampIntoScreen(params: WindowManager.LayoutParams) {
        val screenW = activity.resources.displayMetrics.widthPixels
        val screenH = activity.resources.displayMetrics.heightPixels
        params.x = if (params.width >= screenW) {
            0
        } else {
            params.x.coerceIn(0, screenW - params.width)
        }
        params.y = if (params.height >= screenH) {
            0
        } else {
            params.y.coerceIn(0, screenH - params.height)
        }
    }

    /** 拖动结束后把窗口吸附到最近的左右边缘（带 8dp 边距）。 */
    private fun snapToEdge(params: WindowManager.LayoutParams) {        val view = petView ?: return
        val screenW = activity.resources.displayMetrics.widthPixels
        val margin = dp(8.0)
        val centerX = params.x + view.width / 2
        params.x = if (centerX < screenW / 2) {
            margin
        } else {
            (screenW - view.width - margin).coerceAtLeast(margin)
        }
        try {
            windowManager?.updateViewLayout(view, params)
        } catch (e: Exception) {
            Log.w(TAG, "吸附悬浮窗失败（可忽略）", e)
        }
    }

    // ─── 展开 / 收起 ──────────────────────────────────────────

    /**
     * 展开或收起桌宠窗口。
     *
     * 收起态只显示头像（约 1/6 屏宽），展开后容纳头像 + 输入框
     * （约 2/5 屏宽）。窗口尺寸必须跟着内容变 —— Android 的悬浮窗
     * 没有逐像素穿透，留大块透明区域会挡住下层 App 的触摸。
     *
     * 与桌面端的差异：桌面端靠鼠标悬停自动展开，手机端由前端
     * 「点头像」触发。
     */
    @Command
    fun setExpanded(invoke: Invoke) {
        val args = invoke.parseArgs(ExpandedArgs::class.java)
        activity.runOnUiThread {
            val params = layoutParams
            val view = petView
            if (params == null || view == null) {
                invoke.reject("NOT_VISIBLE")
                return@runOnUiThread
            }
            try {
                expanded = args.expanded
                val (width, height) = if (expanded) expandedSize() else collapsedSize()

                // 以中心为锚点缩放：直接改宽高会让窗口往右下角长，
                // 视觉上像「跳」了一下。这里保持中心不动。
                val centerX = params.x + view.width / 2
                val centerY = params.y + view.height / 2

                params.width = width
                params.height = height
                params.x = centerX - width / 2
                params.y = centerY - height / 2
                clampIntoScreen(params)

                // ── 输入法：只有展开态才让窗口可获焦 ──────────────────
                // FLAG_NOT_FOCUSABLE 的窗口永远收不到输入法：IME 只服务于
                // 持有输入焦点的窗口。收起态保持 NOT_FOCUSABLE（不抢焦点、
                // 不挡返回键）；展开态必须去掉它，否则点输入框毫无反应。
                //
                // 代价：展开期间悬浮窗会持有输入焦点，返回键也会先给它。
                // 因此收起时一定要把标志加回去（见 else 分支），
                // 不能只在展开时改一次。
                if (expanded) {
                    params.flags =
                        params.flags and WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE.inv()
                    // ADJUST_RESIZE：键盘弹出时把窗口往上顶，输入框不被遮住
                    params.softInputMode =
                        WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE or
                        WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE
                } else {
                    params.flags =
                        params.flags or WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                    params.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_STATE_UNCHANGED
                }

                windowManager?.updateViewLayout(view, params)

                // 展开后主动请求焦点，页面里的输入框才能拿到 IME
                if (expanded) view.requestFocus()

                // 先把权威尺寸推给页面（页面据此重算 scale 与内容高度），
                // 再通知展开态。顺序不能反：页面收到展开态会立刻按新布局
                // 量高度，那时 scale 必须已经是新的。
                notifyMetrics(params, view)

                // 通知页面切换布局（头像态 vs 完整态）
                notifyWeb(view as? WebView, "pet-expanded-changed", JSObject().apply { put("expanded", expanded) })
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "切换展开状态失败", e)
                invoke.reject("切换展开状态失败: ${e.message}")
            }
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
                // width <= 0 表示「只改高度，宽度保持不变」。
                //
                // 宽度**必须**由原生独占：它是按屏幕比例算出来的，前端只负责
                // 内容高度。前端曾经回传 window.innerWidth 当宽度，而原生刚
                // updateViewLayout 完时 WebView 的视口还没跟上，那个值是滞后的
                // ——于是页面会把刚展开的窗口又缩回收起态，表现就是「内容宽度
                // 总是很小、展开后一大片空白、瞎点几下又莫名其妙好了」。
                if (args.width > 0) {
                    params.width = dp(args.width.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                }
                params.height = dp(args.height.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                clampIntoScreen(params)
                windowManager?.updateViewLayout(view, params)
                notifyMetrics(params, view)
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
     * 评估 JS（在悬浮窗 WebView 里执行）。
     *
     * 现在 WebView 就是主 WebView，`invoke()` 可用，大部分通信应直接走
     * Tauri 命令。这里保留 `evaluateJavascript` 是为了传递那些不方便
     * 走 IPC 的原生事件（如展开状态变更）。
     *
     * ## 为什么必须把 WebView 当参数传进来
     *
     * 早先这里读的是成员 `petView`，于是 [restoreWebViewToActivity] 里
     * 「先 `petView = null` 再 `view.post { notifyWeb(...) }`」的写法会
     * 让事件**永远发不出去**——post 执行时 petView 已经是 null。
     * 表现就是页面永远不知道自己已经回到 Activity：仍按悬浮窗布局渲染
     * （看起来只有小小一块），也不会切回聊天页。
     * 显式传参后就不依赖成员变量的时序了。
     */
    private fun notifyWeb(view: WebView?, function: String, detail: JSObject) {
        if (view == null) return
        val payload = detail.toString().replace("\\", "\\\\").replace("'", "\\'")
        val js =
            "window.dispatchEvent(new CustomEvent('$function',{detail:JSON.parse('$payload')}))"
        try {
            view.evaluateJavascript(js, null)
        } catch (e: Exception) {
            Log.w(TAG, "通知页面失败: $function", e)
        }
    }

    /**
     * 窗口**实际**宽度（物理 px）。
     *
     * 优先取 View 的布局尺寸而不是 `params.width`：系统可能因为 insets /
     * 多窗口对窗口做过调整，`params` 里记的只是我们请求的值。前端要的是
     * 「WebView 现在到底多宽」，那必须以实际布局为准。
     */
    private fun actualWidthPx(params: WindowManager.LayoutParams, view: View?): Int =
        if (view != null && view.width > 0) view.width else params.width

    /**
     * 逻辑画布 → 窗口的缩放系数，与前端 `transform: scale()` 用的值一致。
     *
     * 前端**不能**自己从 `window.innerWidth` 推这个值：原生改完窗口尺寸后
     * WebView 的视口要过一会儿才跟上，这中间读到的宽度是滞后的，算出来的
     * 系数偏小 → 内容只占窗口一角、展开后一大片空白，而且要等下一次
     * resize 事件才自愈（用户感受就是「瞎点几下又莫名其妙好了」）。
     *
     * 原生手里有权威的实际尺寸，因此由原生算好推给前端。
     */
    private fun currentScale(params: WindowManager.LayoutParams, view: View?): Double =
        actualWidthPx(params, view) / density.toDouble() / PET_LOGICAL_WIDTH

    /**
     * 把窗口几何推给页面（`pet-metrics`）。
     *
     * 每次窗口尺寸变化后都要调：show / setExpanded / setSize。
     * 页面据此更新缩放系数，并重新回报内容高度。
     *
     * 注意这只是**快路径**：原生 → 页面的事件在搬运/收回前后并不可靠，
     * 页面还会轮询 `status` 命令拿同样的数据（见 [status]）。
     */
    private fun notifyMetrics(params: WindowManager.LayoutParams, view: View?) {
        notifyWeb(
            view as? WebView,
            "pet-metrics",
            JSObject().apply {
                put("scale", currentScale(params, view))
                put("width", actualWidthPx(params, view) / density.toDouble())
                put("height", params.height / density.toDouble())
            }
        )
    }

    // ─── 生命周期 ─────────────────────────────────────────────

    /**
     * App 退到后台时，保持桌宠的 WebView 继续运行。
     *
     * ## 为什么真正起作用的是 [startKeepAlive] 而不是这里
     *
     * 宿主 Activity 一旦 onPause，`WryActivity.onPause()` 会**无条件**
     * 调用 `mWebView.onPause()`（wry 0.55.1 `WryActivity.kt`），而搬进
     * 悬浮窗的正是这个 WebView —— 于是桌宠在桌面上静止不动：渲染停、
     * `requestAnimationFrame` 停，连 `evaluateJavascript` 都不再执行
     * （原生派发的 `pet-attached` 等事件会直接丢失）。
     *
     * 直觉上应该在插件的 onPause 钩子里补一次 `onResume()`，但**这条路
     * 在 Tauri 2.11.1 上走不通**：`PluginManager.onPause/onResume/onStop`
     * 唯一的上游是 `TauriLifecycleObserver`，而它只被定义、
     * **从未被 `addObserver()` 注册**（见 `mobile/android-codegen/TauriActivity.kt`）。
     * 也就是说这两个覆写目前在真机上根本不会被调用。
     *
     * 保留它们是为了将来 Tauri 补上注册后能立即生效；当下真正兜底的是
     * [startKeepAlive] 的 500ms 轮询——它不依赖任何生命周期回调，
     * 只要还在悬浮窗里就把 WebView 拉回运行态。
     */
    override fun onPause() {
        super.onPause()
        val wv = petView as? WebView ?: return
        if (!petDetached) return
        wv.post { resumePetWebView(wv, "onPause") }
    }

    /** App 回到前台时同样确保 WebView 处于运行状态。 */
    override fun onResume() {
        super.onResume()
        val wv = petView as? WebView ?: return
        if (!petDetached) return
        wv.post { resumePetWebView(wv, "onResume") }
    }

    /**
     * 唤醒桌宠 WebView。
     *
     * `onResume()` 恢复渲染与 JS 执行，`resumeTimers()` 恢复被
     * `pauseTimers()` 全局停掉的定时器——两者都要，只调一个不够：
     * WebView 的暂停是「渲染」与「定时器」两套独立机制。
     */
    private fun resumePetWebView(wv: WebView, from: String) {
        try {
            wv.onResume()
            wv.resumeTimers()
            // 保活轮询每 500ms 走一次，别刷日志
            if (from != "keep-alive") Log.d(TAG, "已唤醒桌宠 WebView（$from）")
        } catch (e: Exception) {
            Log.w(TAG, "唤醒桌宠 WebView 失败（$from，可忽略）", e)
        }
    }

    /**
     * Activity 销毁时必须移除悬浮窗。
     *
     * 否则 overlay 会留在屏幕上成为「僵尸窗口」：宿主 Activity 已经没了，
     * 用户却还能看到那个宠物，且无法通过 App 关闭它。
     *
     * 用 [detachPetView] 而非 [restoreWebViewToActivity]：Activity 正在销毁，
     * 把 WebView 装回内容视图没有意义，反而可能在销毁流程里制造新引用。
     */
    override fun onDestroy(activity: AppCompatActivity) {
        activity.runOnUiThread { detachPetView() }
        super.onDestroy(activity)
    }
}
