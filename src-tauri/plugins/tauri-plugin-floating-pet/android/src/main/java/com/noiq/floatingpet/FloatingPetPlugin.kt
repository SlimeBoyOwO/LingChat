package com.noiq.floatingpet

import android.annotation.SuppressLint
import android.app.Activity
import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.Display
import android.view.Gravity
import android.view.MotionEvent
import android.view.Surface
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
 * 逻辑画布排版，再由前端 `transform: scale(window.innerWidth / 210)`
 * 整体等比缩放到窗口大小。
 *
 * 这样做的理由：
 * - 布局只有一套（与桌面端完全相同），不会在小窗里错位、溢出、点不到
 * - 文字/按钮/输入框随窗口等比缩放，小窗下不会「挤成一团」
 * - 页面内容恰好铺满逻辑画布 → 窗口里没有大块透明区域
 *
 * 代价：文字绝对大小与窗口宽度成正比，因此展开态不能太窄。
 */
private const val FLOATING_LOGICAL_WIDTH = 210.0
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
 * 展开态取 0.6 而不是 2/5：逻辑宽 210dp 缩到 0.6×360=216dp 时
 * 缩放系数约 1.03（内容不再被缩小），15px 的字渲染成约 15.4px；
 * 2/5 屏宽（144dp）只有 0.69 倍，字会小到看不清。
 */
private const val COLLAPSED_WIDTH_RATIO = 1.0 / 6.0
private const val EXPANDED_WIDTH_RATIO = 0.6

/**
 * 窗口高度与宽度之比 —— 直接由逻辑尺寸推出，保证原生给的初始尺寸
 * 与前端按同一套常量算出的内容高度一致，避免「先给一个错的高度、
 * 前端再纠正一次」造成的闪动。
 *
 * 收起态只有头像（210/210 = 1，正方形）；展开态是头像 + 输入框
 * （280/210 ≈ 1.3333）。气泡出现时窗口高度由前端通过 `set_size`
 * 再撑高，不在这里预留。
 */
private const val COLLAPSED_HEIGHT_RATIO = COLLAPSED_LOGICAL_HEIGHT / FLOATING_LOGICAL_WIDTH
private const val EXPANDED_HEIGHT_RATIO = EXPANDED_LOGICAL_HEIGHT / FLOATING_LOGICAL_WIDTH

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
 *
 * 注意轮询本身很便宜，真正贵的是轮询里那次 `WebView.onResume()`：
 * 它只在「确实被暂停过」时才调一次，见 `petNeedsResume`。
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
 * 收回后延迟清理 [FloatingPetPlugin.pendingAttachNotify] 引用的时长（毫秒）。
 *
 * 只在「Activity 一直没 resume、补发事件始终没送出去」时兜底，
 * 防止那个 WebView 引用一直挂着。
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
 * 桌宠搬进悬浮窗后，把 App 自己退到后台的延迟（毫秒）。
 *
 * 不能同步退：`pet-detached` / `pet-metrics` 是 `evaluateJavascript`
 * 异步投递的，WebView 一旦因宿主 Activity 进后台被 `onPause`，
 * 未执行的那几条会被**整体丢弃**，页面就永远停在桌面端布局里。
 * 让它们先落地，再退后台。
 */
private const val BACKGROUND_DELAY_MS = 400L

/**
 * 单击与拖拽的判定阈值（dp）：按下到抬起位移超过它就算拖动，不触发点击。
 *
 * 取 16dp 而不是 Android 默认的 8dp。这里判定的是「整个窗口要不要跟着
 * 手指走」，不是滚动，因此容差该给得比 `ViewConfiguration` 的 touchSlop
 * 宽：手指点按时天然会带几 dp 位移，8dp 会把大量正常点按判成拖动
 * ——表现就是「单击经常没反应，窗口还会被带偏一点」。
 */
private const val TAP_SLOP_DP = 16.0

/**
 * 显示器变化回调 → 真正重排之间的等待（毫秒）。
 *
 * 旋转时显示状态不会立刻落定：回调到达时 `DisplayInfo` 可能还是中间态，
 * 立刻读会拿到一个错误尺寸，把桌宠夹进一个不存在的屏幕里。等一拍再读。
 *
 * 这段时间内重复到达的回调会被合并（见 [FloatingPetPlugin.displayListener]）。
 */
private const val CONFIG_SETTLE_MS = 120L


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

    /**
     * 逻辑画布高度（dp，**未缩放**）。
     *
     * `> 0` 时原生忽略 [height]，自己按 `逻辑高度 × 当前缩放系数` 算实际高度。
     *
     * 这是页面回报高度的**推荐口径**。页面按「实际 dp」回报时，那个数字是
     * 「逻辑高度 × 页面手里的缩放系数」——一旦页面手里的系数过期（原生刚
     * 改完尺寸、`pet-metrics` 还没送达），它就会把一个和窗口宽度不匹配的
     * 高度写进窗口：宽度已经展开、高度还是收起态，于是「展开后一大片空白」。
     * 改报逻辑高度后，窗口高度与窗口宽度**在构造上**由原生保证一致。
     */
    var logicalHeight: Double = 0.0
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

    /**
     * 宿主 Activity 当前是否处于 resumed 态（由 [ensureLifecycleCallbacks] 维护）。
     *
     * 只有 Activity 处于 paused/stopped 时 wry 才会调用 `mWebView.onPause()`，
     * 也才需要我们补一次 `onResume()`。见 [petNeedsResume]。
     */
    private var hostResumed = true

    /**
     * WebView 是否「被 wry 暂停过、还没补唤醒」。
     *
     * ## 为什么需要这个标记（性能）
     *
     * 早先的保活轮询是**每 500ms 无条件**调一次
     * `WebView.onResume()` + `resumeTimers()`。这两个调用都不是空操作：
     * `onResume()` 会走到 `AwContents.onResume()` → 触发一次重绘/重排，
     * 于是**前台使用时**（Activity 明明还 resumed、wry 根本没暂停过它）
     * 也在以每秒 2 次的频率强制刷新 WebView —— 用户感受就是
     * 「启动和使用时都变卡了」。
     *
     * 事实上 WebView 一旦被唤醒就会一直跑，直到下一次 `onPause()`；
     * 因此每个「暂停周期」只需要补唤醒**一次**。这里在 Activity paused 时
     * 置位、补唤醒后清零，把每秒 2 次的重绘降到「每次切后台 1 次」。
     */
    private var petNeedsResume = false

    private val keepAliveTick = object : Runnable {
        override fun run() {
            if (!keepAliveRunning) return
            try {
                // 只在「wry 刚把它暂停过」时补唤醒一次（见 petNeedsResume）。
                if (petDetached && petNeedsResume) {
                    (petView as? WebView)?.let {
                        resumePetWebView(it, "keep-alive")
                        petNeedsResume = false
                    }
                }
                if (petDetached) {
                    // ── 屏幕尺寸变了就重排 ────────────────────────────
                    //
                    // 旋转的**快路径**是 [displayListener]（约 120ms 响应）。
                    // 这里是**不依赖系统回调是否送达**的兜底：本项目已经反复
                    // 踩到「系统回调不保证送达」（`pet-detached` / `pet-attached`
                    // 都丢过）。那条回调一旦丢掉，桌宠就会一直停在旧屏幕的坐标
                    // 系里 —— 竖屏贴下沿的 `y` 在横屏里远大于屏高，整个窗口跑到
                    // 屏幕外，而用户没有任何办法把它拉回来。
                    //
                    // 判据与代价见 [maybeReapplyForScreenChange]。
                    if (!maybeReapplyForScreenChange()) {
                        // 没变就顺带重推一次窗口几何：前端靠它算缩放系数，而
                        // WebView 的视口在原生改完尺寸后会滞后一会儿，自算必然
                        // 出错。低频重推让页面即使漏掉某次事件也能在半秒内自愈。
                        layoutParams?.let { notifyMetrics(it, petView) }
                    }
                }
            } catch (t: Throwable) {
                // 轮询体绝不能抛出：Handler 里未捕获的异常会直接杀掉进程
                Log.w(TAG, "保活轮询出错（已忽略）", t)
            }
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

    /**
     * 「进出悬浮窗」的轮次号，每次 [show] / [restoreWebViewToActivity] 递增。
     *
     * ## 为什么需要它
     *
     * 收回时 [restoreWebViewToActivity] 会在 0/300/1000/3000ms 各重发一次
     * `pet-attached`（见 [PET_ATTACHED_RETRY_DELAYS_MS]）。那串重试是为
     * 「用户是在别的 App 里收回的、WebView 当时收不到 JS」准备的，但它在
     * **用户很快又重新进悬浮窗**时变成了毒药：
     *
     * ```
     * t=0.0  用户点 ✕ 收回 → 排队 [0, 300, 1000, 3000]
     * t=1.5  用户又点了「启动桌宠」→ 重新搬进悬浮窗，页面重新挂载
     * t=3.0  最后一条陈旧的 pet-attached 到达
     *        → 页面把它当成「用户收回了桌宠」
     *        → router.push('/chat') + 停掉轮询（不可逆）
     *        → 悬浮窗里变成聊天页，角色凭空消失
     * ```
     *
     * 每个延迟任务在**入队时**记下当时的轮次，执行前比对：轮次变了就说明
     * 期间已经重新进出过一次，这条事件属于上一轮，直接丢弃。
     *
     * 这样「反复切来切去」就不会再把用户踢回聊天页。
     */
    private var petModeEpoch = 0

    /** [ensureLifecycleCallbacks] 的幂等标记。 */
    private var lifecycleCallbacksRegistered = false

    /** [ensureDisplayListener] 的幂等标记。 */
    private var displayListenerRegistered = false

    /** 上一次已知的屏幕物理宽度。旋转后用来判断「桌宠原本贴哪一边」。 */
    private var lastScreenW = 0

    /** 上一次已知的屏幕物理高度。旋转后用来换算纵向的相对位置。 */
    private var lastScreenH = 0

    /** [displayListener] 的延时体：把「回调到达」与「真正重排」错开，见 [CONFIG_SETTLE_MS]。 */
    private val reapplyRunnable = Runnable { maybeReapplyForScreenChange() }

    /**
     * 显示器变化回调（旋转 / 分屏 / 折叠屏展开 / 分辨率或刷新率切换）。
     *
     * ## 为什么必须有
     *
     * `MainActivity` 在 manifest 里声明了
     * `android:configChanges="orientation|screenSize|screenLayout|smallestScreenSize|..."`
     * —— 所以旋转**不会重建 Activity**。这对悬浮窗是好事（插件实例、
     * WebView、窗口全都活着），但也意味着**没有任何人**会去处理旋转之后的事：
     *
     * - 悬浮窗的 `x` / `y` 是**屏幕坐标系里的绝对值**，而旋转会把屏幕宽高
     *   对调。竖屏 360×802 里 `y = 700`（贴着屏幕下沿）的桌宠，转到横屏
     *   802×360 之后 `y` 仍然是 700 > 360 —— 整个窗口跑到屏幕外面。
     *   用户看到的就是「一转屏桌宠就没了」。
     * - 尺寸基准是屏幕**短边**（见 [screenBasisDp]），旋转后要按新的短边
     *   重算（竖屏 360×802 与横屏 802×360 的短边都是 360，通常不变；
     *   但分屏 / 折叠屏展开会变）。
     *
     * 这里在尺寸变化后重排：按当前展开态重算尺寸 → 把位置映射到新屏幕
     * （左右保留原来那一边、上下保留相对位置）→ 重新布局 → 把新的权威
     * 几何推给页面。具体见 [reapplyWindowAfterConfigChange]。
     *
     * ## 为什么不是 `ComponentCallbacks.onConfigurationChanged`
     *
     * 那个回调**只在 Activity 可见时才送达**。而桌宠浮在桌面上时，宿主
     * Activity 已经被 `moveTaskToBack(true)` 退到后台 —— 恰恰是最需要它的
     * 那个场景，它**永远不会来**。这条不是理论推演：它就是「旋转后桌宠
     * 跑出屏幕」一直修不掉的原因之一（另一个是读数本身取错了源，
     * 见 [screenSizePx]）。
     *
     * `DisplayManager` 的监听走**进程级**的显示器回调，不看 Activity 可不可见，
     * 只要进程活着就会收到（悬浮窗有前台服务保活）。
     *
     * ## 这个回调不只管旋转
     *
     * 亮度、刷新率、分辨率变化都会触发它，所以**不能**无条件重排 ——
     * 无条件重排会把桌宠重新吸附到边缘，用户正拖着它时会被直接拽走。
     * 真正决定要不要动的是 [maybeReapplyForScreenChange] 里的尺寸比较。
     */
    private val displayListener = object : DisplayManager.DisplayListener {
        override fun onDisplayAdded(displayId: Int) = Unit

        override fun onDisplayRemoved(displayId: Int) = Unit

        override fun onDisplayChanged(displayId: Int) {
            if (displayId != Display.DEFAULT_DISPLAY) return
            // 旋转时这个回调会连着来好几次，且显示状态未必已经落定。
            // 撤掉上一次的待办、只保留最后那次 —— 合成一拍。
            keepAliveHandler.removeCallbacks(reapplyRunnable)
            keepAliveHandler.postDelayed(reapplyRunnable, CONFIG_SETTLE_MS)
        }
    }

    /**
     * 屏幕尺寸**真的变了**才重排。
     *
     * 判据是 [lastScreenW] / [lastScreenH]：它们表示「当前 x/y 是按哪块屏幕
     * 算出来的」，[clampIntoScreen] / `show()` / [reapplyWindowAfterConfigChange]
     * 都会写。两者与实时读数不一致 == 屏幕变过、但窗口还没跟着重排。
     *
     * 代价：每 500ms 一次 [screenSizePx] 读数（本地调用，可忽略）。
     *
     * @return 是否真的重排了 —— [keepAliveTick] 靠它决定要不要顺手补推几何。
     */
    private fun maybeReapplyForScreenChange(): Boolean {
        if (!petDetached) return false
        return try {
            val (curW, curH) = screenSizePx()
            val changed = lastScreenW > 0 && lastScreenH > 0 &&
                (curW != lastScreenW || curH != lastScreenH)
            if (changed) reapplyWindowAfterConfigChange()
            changed
        } catch (t: Throwable) {
            Log.w(TAG, "屏幕尺寸变化后重排失败（可忽略）", t)
            false
        }
    }

    /** 注册显示器回调（幂等）。只在桌宠真的在悬浮窗里时注册。 */
    private fun ensureDisplayListener() {
        if (displayListenerRegistered) return
        displayListenerRegistered = true
        try {
            displayManager().registerDisplayListener(displayListener, keepAliveHandler)
        } catch (t: Throwable) {
            Log.w(TAG, "注册显示器回调失败（可忽略）", t)
        }
    }

    /** 注销显示器回调。桌宠不在悬浮窗里时没必要继续收系统广播。 */
    private fun releaseDisplayListener() {
        if (!displayListenerRegistered) return
        displayListenerRegistered = false
        try {
            displayManager().unregisterDisplayListener(displayListener)
        } catch (t: Throwable) {
            Log.w(TAG, "注销显示器回调失败（可忽略）", t)
        }
    }

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
     * 目前用到两件事：
     * - `onActivityResumed`：把收回时没送达的 `pet-attached` 补发给页面。
     *   这条事件一旦丢失，页面就会一直停在悬浮窗布局。
     * - `onActivityPaused` / `onActivityResumed`：维护 [hostResumed] /
     *   [petNeedsResume]，让保活轮询只在真的被暂停过之后补唤醒一次，
     *   而不是每 500ms 无条件刷新 WebView（那是卡顿的来源）。
     *
     * 注意顺序：`WryActivity.onPause()` 是**先** `super.onPause()`（其中会分发
     * 生命周期回调）**后**才 `mWebView.onPause()`。所以回调里不能立刻
     * `onResume()`——那会被紧随其后的 `mWebView.onPause()` 覆盖掉。
     * 这里只置标记，真正的唤醒留给 500ms 后的 [keepAliveTick]。
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
                        if (a !== activity) return
                        hostResumed = true
                        petNeedsResume = false
                        val wv = pendingAttachNotify ?: return
                        pendingAttachNotify = null
                        Log.i(TAG, "Activity 已回到前台，补发 pet-attached")
                        notifyWeb(wv, "pet-attached", JSObject())
                    }

                    override fun onActivityPaused(a: Activity) {
                        if (a !== activity) return
                        hostResumed = false
                        // 紧随其后 wry 会 mWebView.onPause()，交给我方轮询补唤醒
                        if (petDetached) petNeedsResume = true
                    }

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
     * 悬浮窗里就持续把 WebView 拉回运行态。
     *
     * ## 但**不能**每轮都真的去唤醒
     *
     * `onResume()` / `resumeTimers()` 在 WebView 内部不是空操作：
     * `onResume()` 会走 `AwContents.onResume()` 并触发重绘。早先每 500ms
     * 无条件调用一次，等于**每秒强制刷新 2 次 WebView**——App 明明在前台、
     * wry 根本没暂停过它，也在被刷。用户感受就是「启动和使用时都变卡了」。
     *
     * WebView 被唤醒后会一直跑，直到下一次 `onPause()`；所以每个暂停周期
     * 只需要补唤醒一次，由 [petNeedsResume] 控制（见该字段的说明）。
     */
    private fun startKeepAlive() {
        if (keepAliveRunning) return
        keepAliveRunning = true
        // 若此刻宿主已在后台，进来就先补一次唤醒
        petNeedsResume = petDetached && !hostResumed
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

    /** `DisplayManager` 系统服务。 */
    private fun displayManager(): DisplayManager =
        activity.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager

    /**
     * 屏幕物理尺寸（px，含系统栏），**当前旋转**下的值。
     *
     * ## 这个读数被证伪过三次 —— 三次都绕回了同一个 Resources
     *
     * 真机现象一：**横屏时桌宠只能停在左半边** —— 往右拖到大约「竖屏宽度」
     * 的位置就停住，右边一大片过不去。
     * 真机现象二：**旋转屏幕后桌宠跑出屏幕**，怎么转都回不来。
     *
     * 两者是**同一个成因**：读到的屏幕尺寸一直停在竖屏。
     *
     * 三版来源，一版比一版像对的：
     *
     * 1. `activity.resources.displayMetrics` —— 直接读 Resources。
     * 2. `getSystemService(WINDOW_SERVICE).maximumWindowMetrics` —— AOSP 的
     *    `WindowMetricsController` 里它就是
     *    `mContext.getResources().getConfiguration().windowConfiguration.maxBounds`，
     *    同一个 Resources 换了个字段，等于没改。
     * 3. `DisplayManager.getDisplay().getRealMetrics()` —— 看着终于离开
     *    Resources 了，其实**又绕了回去**（`android/view/Display.java`）：
     *
     * ```java
     * mDisplayInfo.getLogicalMetrics(outMetrics, ...);   // ← 这里已经是当前旋转的尺寸
     * final int rotation = getLocalRotation();           // ← 读的却是 mResources 的配置
     * if (rotation != mDisplayInfo.rotation) {
     *     adjustMetrics(outMetrics, mDisplayInfo.rotation, rotation);  // ← 又交换回竖屏
     * }
     * ```
     *
     * `mResources` 就是这个 `Display` 关联的 Resources —— `DisplayManager` 由
     * `activity.getSystemService(DISPLAY_SERVICE)` 拿到，所以它就是 Activity 的。
     * 悬浮窗里 Activity 在后台，它的旋转停在「进入悬浮窗那一刻」，于是
     * **已经正确的横屏尺寸被 `adjustMetrics` 又交换回竖屏**，边界回到老样子。
     *
     * （`shouldReportMaxBounds()` 为真时更直接：走 `getMaxBoundsMetrics`，
     * 那本来就是拿 `mResources.getConfiguration()` 算的。）
     *
     * ## 正确的来源：两个直读 DisplayInfo 的读数
     *
     * `Display.getRotation()` 和 `Display.getMode()` 都只读 `mDisplayInfo`
     * 自己的字段，**不经过任何 Resources / DisplayAdjustments**：
     *
     * - `getRotation()` → `mDisplayInfo.rotation`，物理旋转，实时
     * - `getMode()` → 物理分辨率，**不随旋转**，所以要用 rotation 自己换宽高
     *
     * 两者组合就是「当前旋转下的屏幕尺寸」，与 App 可不可见完全无关。
     *
     * 不用 `getRealMetrics` / `getSize` / `WindowMetrics` —— 它们全都经过
     * Resources 或 DisplayAdjustments，在这个场景下都会拿到滞后的旋转。
     *
     * 读数失败时退回 `resources.displayMetrics`：宁可边界偏小，也不能让
     * 拖拽 / 吸附整条路径抛异常。
     */
    private fun screenSizePx(): Pair<Int, Int> {
        try {
            val display = displayManager().getDisplay(Display.DEFAULT_DISPLAY)
            if (display != null) {
                val mode = display.mode
                val pw = mode?.physicalWidth ?: 0
                val ph = mode?.physicalHeight ?: 0
                if (pw > 0 && ph > 0) {
                    val rotated = display.rotation == Surface.ROTATION_90 ||
                        display.rotation == Surface.ROTATION_270
                    return if (rotated) ph to pw else pw to ph
                }
            }
            Log.w(TAG, "DisplayManager 未返回可用的显示器尺寸，退回 resources.displayMetrics")
        } catch (t: Throwable) {
            Log.w(TAG, "读取屏幕尺寸失败，退回 resources.displayMetrics", t)
        }
        val dm = activity.resources.displayMetrics
        return dm.widthPixels to dm.heightPixels
    }

    /**
     * 屏幕调试信息（只用于日志）。
     *
     * 真机排查「转屏后位置不对」时，光看 `screenSizePx()` 的结果分不清是
     * 「旋转没读到」还是「读到但算错了」。把 rotation 与物理分辨率一起打出来，
     * 一眼就能判断。
     */
    private fun screenDebugInfo(): String = try {
        val d = displayManager().getDisplay(Display.DEFAULT_DISPLAY)
        val m = d?.mode
        "rotation=${d?.rotation}, mode=${m?.physicalWidth}x${m?.physicalHeight}"
    } catch (t: Throwable) {
        "unavailable"
    }

    /** 屏幕宽度（dp）。见 [screenSizePx] —— 刻意不用 `resources.displayMetrics`。 */
    private fun screenWidthDp(): Double = screenSizePx().first / density.toDouble()

    /** 屏幕高度（dp）。见 [screenSizePx]。 */
    private fun screenHeightDp(): Double = screenSizePx().second / density.toDouble()

    /**
     * 尺寸基准：屏幕的**短边**（dp）。
     *
     * 窗口尺寸必须由短边推出来，否则横屏必崩：
     *
     * ```
     * 横屏 802×360 dp，展开态宽 = 802 × 0.6 = 481dp
     *   高 = 481 × (280/240) = 561dp  >  屏幕高 360dp
     * ```
     *
     * 窗口比屏幕还高 201dp，`clampIntoScreen` 只能把它按到 y=0，
     * 结果是宠物下半身和整个输入带被挤到屏幕外，用户看到的正是
     * 「展开后一片空白 + 输入框不见了」。
     *
     * 取短边后：竖屏 min(360,802)=360 → 216×252（与旧行为逐位相同，无回归）；
     * 横屏 min(802,360)=360 → 同样是 216×252，稳稳落在屏幕里。
     *
     * 这也让桌宠的**物理尺寸与方向无关** —— 转屏时宠物不会突然变大变小。
     */
    private fun screenBasisDp(): Double = minOf(screenWidthDp(), screenHeightDp())

    /** 收起态尺寸：宽度约 1/6 屏（短边）。 */
    private fun collapsedSize(): Pair<Int, Int> {
        val w = (screenBasisDp() * COLLAPSED_WIDTH_RATIO * petScale)
            .coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
        val h = (w * COLLAPSED_HEIGHT_RATIO).coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
        return dp(w) to dp(h)
    }

    /** 展开态尺寸：宽度约 3/5 屏（短边）。 */
    private fun expandedSize(): Pair<Int, Int> {
        val w = (screenBasisDp() * EXPANDED_WIDTH_RATIO * petScale)
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
                put("scale", if (params != null) currentScale(params) else 1.0)
                put(
                    "width",
                    if (params != null) authoritativeWidthPx(params) / density.toDouble() else 0.0
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
                val previousView = petView
                if (previousView != null) {
                    restoreWebViewToActivity(notifyPage = false)
                }

                petScale = args.scale.coerceIn(0.5, 2.0)

                val webView = findMainWebView()
                if (webView == null) {
                    // 上面的幂等分支已经把 WebView 还给 Activity 了。此时页面
                    // 如果还停在悬浮窗布局，就会**永远**停下去：页面轮询看到
                    // `detached=false`，但它从没观察到过 `detached=true`
                    // （sawDetached 仍为 false），不会自愈。
                    // 补发一次 pet-attached，把页面送回桌面布局。
                    if (previousView != null) {
                        notifyWeb(previousView as? WebView, "pet-attached", JSObject())
                    }
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
                // 开新一轮（见 [petModeEpoch]）：作废上一轮所有排队中的
                // pet-attached 重试，否则它们会在几秒后落进这一轮的页面里，
                // 把「刚进来」误判成「已收回」。
                petModeEpoch += 1
                // 同理：上一轮收回时留下的补发引用也必须清掉，不然 Activity
                // 一 resume 就会补发一条陈旧的 pet-attached（见
                // [ensureLifecycleCallbacks] 的 onActivityResumed）。
                pendingAttachNotify = null

                val root = activity.findViewById<ViewGroup>(android.R.id.content)
                root?.removeView(webView)

                // ── 2. 给 Activity 塞占位页，避免白屏 ──
                activity.setContentView(buildPlaceholderView())

                // ── 3. 把 WebView 放进悬浮窗 ──
                //
                // ── 为什么必须有 FLAG_NOT_TOUCH_MODAL ──────────────────
                //
                // 这是「桌宠外面一整屏的透明区域都能被摸、摸着还能把桌宠拖走」
                // 这个老问题的**真正**成因。Android 官方对它的定义是：
                //
                //   Window flag: even when this window is focusable (its
                //   FLAG_NOT_FOCUSABLE is not set), allow any pointer events
                //   outside of the window to be sent to the windows behind it.
                //   **Otherwise it will consume all pointer events itself,
                //   regardless of whether they are inside of the window.**
                //
                // 也就是说：**不给这个 flag 的窗口，可触摸区域不是它自己那块
                // 矩形，而是整块屏幕。** 窗口外的触摸也照样投递给它 —— 于是
                // [buildPetTouchListener] 的 ACTION_DOWN 被触发、窗口跟着手指走，
                // 用户看到的就是「外面很大一片区域可以被触摸来拖动桌宠」，
                // 而且**在别的应用里同样如此**（那里根本没有我们的 Activity）。
                //
                // 为什么偏偏**展开态**才明显：收起态带着 FLAG_NOT_FOCUSABLE，
                // 系统对不可获焦窗口的处理要收敛得多；而 [setExpanded] 展开时会把
                // FLAG_NOT_FOCUSABLE 摘掉（为了能弹输入法），窗口一旦可获焦，
                // 「吃掉整屏触摸」这条就完整生效了 —— 正是用户观察到的
                // 「缩小的时候正常，放大就不正常」。
                //
                // 修法就是把它**常驻**（收起态也留着，无害）：窗口的可触摸区域
                // 从此严格等于窗口自己那块矩形，窗口外的触摸原样交给下层应用。
                // 注意 [setExpanded] 只做 `flags or / and inv(FLAG_NOT_FOCUSABLE)`，
                // 不重建 flags，所以本 flag 在两个形态下都不会被弄丢。
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
                    // FLAG_NOT_TOUCH_MODAL：**落在窗口外的触摸必须交给下层窗口**
                    // FLAG_LAYOUT_NO_LIMITS：允许气泡绘制到窗口外/贴边
                    // FLAG_HARDWARE_ACCELERATED：WebView 需要硬件加速渲染
                    WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                        WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
                        WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED,
                    PixelFormat.TRANSLUCENT
                ).apply {
                    gravity = Gravity.TOP or Gravity.START
                    x = dp(args.x)
                    y = dp(args.y)

                    // ── 明确声明「本窗口自己处理 insets」──────────────────
                    //
                    // 窗口一旦**可获焦**（展开态会摘掉 FLAG_NOT_FOCUSABLE），
                    // 系统就会开始给它算 window insets：状态栏、导航栏、挖孔。
                    // 默认行为是把这些 inset 当成 padding 加到内容视图上，
                    // 于是 WebView 比窗口小一圈，窗口半透明的底透出来 ——
                    // 表现就是「四周一圈透明」，而且那圈透明区照样吃触摸。
                    //
                    // 收起态因为一直是 FLAG_NOT_FOCUSABLE，从不进这套机制，
                    // 所以「缩小的时候正常、放大就不正常」。
                    //
                    // fitInsetsTypes = 0 明确告诉系统：别给我加任何 inset，
                    // 我要的就是整个窗口。API 30 以下用等价的 systemUiVisibility。
                    //
                    // ⚠️ 这两行**必须在 apply 块内**：它们是
                    // WindowManager.LayoutParams 的成员，出了这个块就没有接收者。
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                        fitInsetsTypes = 0
                    } else {
                        @Suppress("DEPRECATION")
                        systemUiVisibility =
                            View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
                            View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                            View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    }
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

                // 记住这次布局所在的屏幕尺寸，并开始监听屏幕配置变化。
                //
                // 旋转会把屏幕宽高**对调**，而窗口的 `x` / `y` 是屏幕坐标系里
                // 的绝对值 —— 竖屏贴着下沿（y 很大）的桌宠，转到横屏后 y 仍然
                // 是那个大值，于是整个窗口跑到屏幕外。没有任何系统回调会替我
                // 们处理这件事（manifest 声明了 configChanges，Activity 不重建），
                // 所以必须自己监听、自己重排。见 [reapplyWindowAfterConfigChange]。
                val (startScreenW, startScreenH) = screenSizePx()
                lastScreenW = startScreenW
                lastScreenH = startScreenH
                ensureDisplayListener()

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

                // ── 把 App 自己退到后台 ────────────────────────────────
                //
                // 桌宠要浮在**桌面 / 其他应用**之上，App 自己必须先让开。
                //
                // 主题里已经配好 `windowIsTranslucent` + `windowShowWallpaper`
                // + `windowBackground=transparent`（窗口透明、显示系统壁纸），
                // 但那只是「窗口透明」。只要 App 还留在前台，它就占着整块屏幕：
                //   - 用户看不到自己的桌面，也点不到别的应用图标
                //   - 那整屏区域仍然归我们的窗口所有，触摸会落到它身上
                // 真机表现就是「桌宠外面有覆盖整个屏幕的一大片区域」。
                //
                // 退到后台后：壁纸立刻可见，桌宠浮在上面；用户从最近任务
                // 切回来时看到的是占位页上那两行引导文案。
                //
                // WebView 此刻已经在 WindowManager 里（不随 Activity 进后台），
                // 加上 PetForegroundService 的前台优先级，渲染与 IPC 都不受影响。
                // 收回时由 [bringActivityToFront] 把任务栈拉回来。
                //
                // **延后 400ms 再退**：上面那两条 `evaluateJavascript`
                // （pet-detached / pet-metrics）是异步投递的，而 WebView 一旦
                // 因宿主 Activity 进后台被 onPause，未执行的 evaluateJavascript
                // 会被整体丢弃 —— 页面就永远停在桌面端布局里。等它落地再退。
                keepAliveHandler.postDelayed(
                    {
                        try {
                            activity.moveTaskToBack(true)
                        } catch (t: Throwable) {
                            Log.w(TAG, "把 App 退到后台失败（可忽略）", t)
                        }
                    },
                    BACKGROUND_DELAY_MS
                )

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
                    // 顺序：**先**把任务栈拉到前台，**再**搬 WebView。
                    //
                    // `restoreWebViewToActivity()` 会把 WebView 重新挂进
                    // Activity 的内容视图；而用户点 ✕ 时人往往在别的 App 里，
                    // Activity 处于 stopped 态。往一个 stopped 的 Activity 里
                    // 搬视图，紧接着它又被拉到前台重新走 resume/布局，
                    // 是最容易出事的组合。先把任务栈叫到前台，让这次搬运
                    // 落在一个正在恢复的 Activity 上。
                    bringActivityToFront()
                    restoreWebViewToActivity()
                    invoke.resolve()
                } catch (t: Throwable) {
                    // 这里必须 catch Throwable：Handler 里逃出去的异常会直接
                    // 杀掉进程（用户看到的就是「点返回就闪退」）。
                    Log.e(TAG, "恢复主界面失败", t)
                    try {
                        invoke.reject("恢复主界面失败: ${t.message}")
                    } catch (ignored: Throwable) {
                        // invoke 可能已随进程状态失效，忽略
                    }
                }
            },
            HIDE_DELAY_MS
        )
    }

    /**
     * 把宿主 Activity 从后台拉到前台。
     *
     * ## 为什么不用 `startActivity`
     *
     * 这一版之前试过两种 `startActivity`，真机上**点 ✕ 都会闪退**：
     *
     * 1. `Intent(activity, activity.javaClass)` + `REORDER_TO_FRONT or SINGLE_TOP`
     * 2. `ACTION_MAIN` + `CATEGORY_LAUNCHER` + 显式组件 + `NEW_TASK`（Launcher 那条）
     *
     * 第 1 种是明确的用法错误：本 App 的 `MainActivity` 是
     * `android:launchMode="singleTask"`，singleTask 的启动语义由系统接管
     * （`ActivityStarter` 会强制补 `NEW_TASK` 并走「复用已有实例 + CLEAR_TOP
     * 式收尾」），与 `REORDER_TO_FRONT` 语义互斥。
     *
     * 第 2 种是标准做法，真机仍然闪退——说明问题不在 flag，而在
     * **`startActivity` 这件事本身**：它会给已有实例投递 `onNewIntent`，
     * 而我们此刻正在同一个消息里搬运 WebView、宿主 Activity 还在后台，
     * 等于把「Activity 被重新拉起」和「视图树正在换根」叠在一起。
     *
     * ## 现在用 `moveTaskToFront`
     *
     * 它走的是**任务栈**而不是 Activity 启动：不构造 Intent、不投递
     * `onNewIntent`、不碰 launchMode、不可能创建第二个实例——正是
     * Launcher / Recents 把 App 切回前台用的那条路。需要 `REORDER_TASKS`
     * 权限（normal 级，manifest 声明即授予）。
     *
     * Activity 已经在前台时这是一个无害的空操作。
     */
    private fun bringActivityToFront() {
        try {
            val am = activity.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
            if (am == null) {
                Log.w(TAG, "拿不到 ActivityManager，无法把任务栈拉到前台")
                return
            }
            am.moveTaskToFront(activity.taskId, ActivityManager.MOVE_TASK_NO_USER_ACTION)
            Log.i(TAG, "已把任务栈拉到前台（taskId=${activity.taskId}）")
        } catch (t: Throwable) {
            // 少数 ROM 限制后台拉起任务栈；失败不致命，用户手动切回来即可
            Log.w(TAG, "把任务栈拉到前台失败（可忽略）", t)
        }
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
        // 开新一轮（见 [petModeEpoch]）：作废上一轮所有排队中的 pet-attached
        // 重试，并把本轮排队的重试标记成「本轮专属」—— 只要用户中途又进了
        // 悬浮窗（轮次再次递增），下面那些延迟任务就会自行作废。
        petModeEpoch += 1
        val epoch = petModeEpoch

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
        // 桌宠不在悬浮窗里了，旋转重排已经没有意义，注销监听
        releaseDisplayListener()

        // 桌宠已收回，不再需要前台优先级
        PetForegroundService.stop(activity)

        if (view != null && petDetached) {
            // ── 必须把 LayoutParams 改回 MATCH_PARENT ──────────────────────
            //
            // `setContentView(view)` **不会**重置 View 的 LayoutParams：实测它
            // 保留了悬浮窗那套尺寸（展开态 216×252dp）。于是 WebView 回到
            // Activity 后视图本身仍然只有悬浮窗那么大，整个 App 被挤在屏幕
            // 左上角一小块里。
            //
            // 真机诊断数据（360×803dp 的屏幕）：
            //     innerW=216 innerH=252 fit=1.000 floating=false
            // 216×252 正是展开态悬浮窗的尺寸 —— 视口没跟上只是表象，
            // 真正的原因是 View 的尺寸压根没被改回来。
            //
            // ⚠️ 但**绝不能**事后写
            //     `view.layoutParams = ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT)`
            //
            // `setContentView(view)` 内部走的是 `contentParent.addView(view)`，
            // 而 contentParent 是 FrameLayout：`addViewInner` 会把 View 的
            // LayoutParams 归一成 `FrameLayout.LayoutParams`。事后塞一个
            // **基类** `ViewGroup.LayoutParams` 进去，下一次 measure 时
            // `FrameLayout.onMeasure` 里的
            //     `final LayoutParams lp = (LayoutParams) child.getLayoutParams();`
            // 会抛 **ClassCastException**。那是主线程未捕获异常 → 直接杀进程。
            //
            // 这就是「点 ✕ 收回就闪退」的真凶：它从第 50 轮（82ef290b 引入这行）
            // 起一直存在，后面几轮改的 startActivity / moveTaskToFront /
            // forceViewportRefresh 都不是病根。
            //
            // 正确做法是走 `setContentView(view, params)` 这个重载：它把参数
            // 交给 `ViewGroup.addView(child, params)`，由 `addViewInner` 用
            // `checkLayoutParams` / `generateLayoutParams` 归一成正确的子类，
            // 既拿到 MATCH_PARENT，又不会留下类型不匹配的坑。
            activity.setContentView(
                view,
                ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            )

            view.setBackgroundColor(Color.TRANSPARENT)

            // 这里**曾经**还调过一次 forceViewportRefresh(view)（已删除）：把高度
            // 改成 `height - 1` 再在下一帧改回 MATCH_PARENT，靠两次尺寸变化逼
            // Chromium 重算 CSS 视口。删掉的理由见下面这段注释。
            //
            // 现在删掉了，两个理由：
            // 1. 那个诊断本身就是错的——「切回去只有左上一角」的真病根是
            //    setContentView 不重置 LayoutParams（见上），上面那行已经修好；
            // 2. 它是在**刚重新挂载的 WebView** 上连续做两次「故意写错尺寸」的
            //    布局，而且是挂在 post 里的延迟任务，真机上点 ✕ 收回会闪退。
            //    收回路径上不该留这种「用错误状态换一次重算」的写法。

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
                    view.postDelayed(
                        {
                            // 轮次变了 == 期间用户又重新进过悬浮窗（或又收回过一次）。
                            // 这条事件属于上一轮，发出去只会让页面把「刚进来」误判成
                            // 「已收回」→ router.push('/chat') 且轮询被停（不可逆）。
                            // 真机表现就是「反复切来切去时悬浮窗卡成聊天页」。
                            if (epoch != petModeEpoch) {
                                Log.i(TAG, "丢弃过期的 pet-attached（轮次已变，用户已重新进出悬浮窗）")
                                return@postDelayed
                            }
                            // Handler 里逃出去的异常会直接杀进程，必须兜住
                            try {
                                notifyWeb(wv, "pet-attached", JSObject())
                            } catch (t: Throwable) {
                                Log.w(TAG, "补发 pet-attached 失败（可忽略）", t)
                            }
                        },
                        delay
                    )
                }
                // 兜底清理：万一 Activity 一直没 resume（例如用户再也没回来），
                // 不要让引用一直挂着。同样按轮次判断，避免把**新一轮**刚记下的
                // 补发引用误清掉。
                keepAliveHandler.postDelayed(
                    { if (epoch == petModeEpoch) pendingAttachNotify = null },
                    KEEP_ALIVE_GRACE_MS
                )
            }

            // 保活轮询到此为止。
            //
            // 早先这里还让轮询多撑 20 秒，理由是「WebView 从 60dp 的悬浮窗
            // 被塞回整屏 Activity 时视口要重算」。那个诊断是错的：真正的
            // 病根是 `setContentView` 不重置 LayoutParams（见上面的说明），
            // 现在已显式改回 MATCH_PARENT。继续撑着只会让 WebView 在
            // Activity 已经前台的情况下被反复唤醒——纯粹的性能损失。
            //
            // 另外，若收回后宿主仍在后台，WebView 会保持 wry 暂停它的状态；
            // 等用户切回来时 `WryActivity.onResume()` 会自己唤醒它，
            // 不需要我们代劳。
            stopKeepAlive()
        }
        petDetached = false
        petNeedsResume = false
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
        petNeedsResume = false
        pendingAttachNotify = null
        releaseDisplayListener()
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
        // ── 底色：不透明的近黑 #101014 ──────────────────────────────
        //
        // 这一页只有**用户主动切回 App 时**才看得到（桌宠浮在桌面上期间 App
        // 自己退在后台，见 show() 末尾的 moveTaskToBack），所以它需要的是一块
        // 读得清字的背板，而不是「透出壁纸」。
        //
        // 这里曾经去掉过这个底色，理由是「主题已经配了 windowIsTranslucent +
        // windowShowWallpaper，刷不透明底色会把壁纸盖掉，而那正是用户反复报的
        // 『外面很大一片区域』」。那个诊断是**错的**：那片区域的真身是悬浮窗缺
        // FLAG_NOT_TOUCH_MODAL 时被它吃掉的整屏触摸（见 show() 里的说明），
        // 与占位页底色无关。去掉底色之后，下面那两行文案直接压在壁纸上，
        // 深色壁纸下几乎看不清 —— 用户反馈「改成透明背景后上面显示的内容都不清晰」。
        //
        // 主题里的 windowIsTranslucent / windowShowWallpaper 保持不动：它们对
        // 占位页之外的行为（进入 /pet 那一瞬间的过渡）仍然有意义。
        val root = android.widget.FrameLayout(context)
        root.setBackgroundColor(Color.parseColor("#101014"))

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
     * 屏幕旋转 / 分屏 / 折叠屏展开之后，把悬浮窗重排到新屏幕里。
     *
     * ## 两个调用点，缺一不可
     *
     * - [displayListener] 的 `onDisplayChanged`（正常路径，延后 [CONFIG_SETTLE_MS]）
     * - [keepAliveTick] 的 500ms 轮询（兜底：那条系统回调丢了也要能自愈）
     *
     * 只留前者是不够的 —— 本项目已经反复踩到「系统回调不保证送达」。
     * 那条回调一旦丢掉，桌宠会永远停在旧屏幕的坐标系里。
     *
     * 两个调用点都必须先过 [maybeReapplyForScreenChange] 的尺寸比较：
     * 本函数一旦执行就会把桌宠**重新吸附到边缘**，无变化时误触发会把
     * 用户正拖着的桌宠拽走。
     *
     * 要点：
     *
     * - **尺寸**按当前展开态重算（基准是屏幕短边，见 [screenBasisDp]）。
     * - **左右**：保留原来贴的那一边。竖屏贴右沿的桌宠，转横屏后应该还在
     *   右沿，而不是因为 `x` 越界被夹到左上角。
     * - **上下**：保留**相对**位置（`y / (屏高 − 窗高)`）。竖屏贴下沿的
     *   桌宠转到横屏后仍在下方，而不是原样带着 `y = 700` 飞出屏幕。
     *
     * 全程在 UI 线程；失败只记日志 —— 旋转是用户高频操作，绝不能因为
     * 一次布局异常把进程带走。
     */
    private fun reapplyWindowAfterConfigChange() {
        val params = layoutParams ?: return
        val view = petView ?: return
        try {
            val (screenW, screenH) = screenSizePx()

            // 旋转前的屏幕尺寸。首次调用时可能还没记录，退回当前值 ——
            // 此时相对位置退化为「不动」，但下面仍会夹回屏幕内。
            val oldW = if (lastScreenW > 0) lastScreenW else screenW
            val oldH = if (lastScreenH > 0) lastScreenH else screenH
            lastScreenW = screenW
            lastScreenH = screenH

            val oldWidth = if (params.width > 0) params.width else 1
            val wasRightHalf = params.x + oldWidth / 2 >= oldW / 2
            val yRatio = if (oldH > params.height) {
                (params.y.toDouble() / (oldH - params.height)).coerceIn(0.0, 1.0)
            } else {
                0.5
            }

            val (width, height) = if (expanded) expandedSize() else collapsedSize()
            params.width = width
            params.height = height

            val margin = dp(8.0)
            params.x = if (wasRightHalf) {
                (screenW - width - margin).coerceAtLeast(margin)
            } else {
                margin
            }
            params.y = ((screenH - height) * yRatio).toInt()
            clampIntoScreen(params)

            windowManager?.updateViewLayout(view, params)
            // 窗口宽度可能变了（分屏 / 折叠屏），必须把新的权威系数推给页面，
            // 否则页面还按旧宽度缩放 —— 那正是「展开后一大片空白」的配方。
            notifyMetrics(params, view)
            Log.i(
                TAG,
                "屏幕尺寸变化：悬浮窗重排为 ${width}x$height @ (${params.x},${params.y})，" +
                    "屏幕 ${screenW}x$screenH（${screenDebugInfo()}）"
            )
        } catch (t: Throwable) {
            Log.w(TAG, "屏幕配置变化后重排悬浮窗失败（可忽略）", t)
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
     *
     * 顺带把当前屏幕尺寸记进 [lastScreenW] / [lastScreenH]：
     * 本函数是所有布局路径的必经之地，是「上一次已知屏幕尺寸」最可靠的
     * 记录点，旋转后靠它判断桌宠原来贴哪一边。
     */
    private fun clampIntoScreen(params: WindowManager.LayoutParams) {
        val (screenW, screenH) = screenSizePx()
        lastScreenW = screenW
        lastScreenH = screenH
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

    /**
     * 把窗口吸附到最近的左右边缘（带 8dp 边距）。
     *
     * 宽度取 `params.width` 而不是 `view.width`：本函数也会在
     * `updateViewLayout` **之前**被调用（收起时要先算好落点再一次性布局），
     * 那时 `view.width` 还是旧值，用它算会吸到错误的位置。
     *
     * @param apply 是否立即 `updateViewLayout`。拖动结束时用 true；
     *   [setExpanded] 里已经在同一次布局里改了尺寸，传 false 少一次遍历。
     */
    private fun snapToEdge(params: WindowManager.LayoutParams, apply: Boolean = true) {
        val (screenW, _) = screenSizePx()
        val margin = dp(8.0)
        val width = params.width
        val centerX = params.x + width / 2
        params.x = if (centerX < screenW / 2) {
            margin
        } else {
            (screenW - width - margin).coerceAtLeast(margin)
        }
        if (!apply) return
        val view = petView ?: return
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

                // 收起后贴边。
                //
                // 收起态窗口只有约 1/6 屏宽，若停在屏幕正中会一直挡着内容；
                // 而「贴边」此前只在拖动结束时做，收起是居中缩放，于是缩完
                // 就停在原地了。
                //
                // apply = false：落点先算好，和尺寸一起在下面那次
                // updateViewLayout 里生效，省一次布局遍历。
                if (!expanded) snapToEdge(params, apply = false)

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
                    // ADJUST_RESIZE：键盘弹出时把窗口往上顶/缩内容区，输入框不被遮住。
                    //
                    // ⚠️ 这里**绝不能**再带 SOFT_INPUT_STATE_VISIBLE。
                    //
                    // 早先写了 `ADJUST_RESIZE or SOFT_INPUT_STATE_VISIBLE`，于是
                    // **一点展开输入法就自动弹出来**，系统随即改写窗口的内容区，
                    // WebView 的真实视口跟着变小 —— 而画布的尺寸只由窗口**宽度**
                    // 推出（`--pet-fit = 窗口宽/240`，高度恒为 `280 × fit`），
                    // 高度被系统改小它完全不知情。结果是画布比视口高 → 溢出 →
                    // 浏览器把内容往上顶 → 宠物顶部被切掉、下方空出一片。
                    //
                    // 这正是「收起态正常、展开态不正常」的唯一结构性差异：
                    // 收起态是 FLAG_NOT_FOCUSABLE，压根进不了这套机制。
                    //
                    // 只留 ADJUST_RESIZE：键盘仍然会在**用户点输入框时**正常弹出
                    // （窗口已可获焦），但不会在展开的一瞬间被系统强行改写尺寸。
                    params.softInputMode =
                        WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
                } else {
                    params.flags =
                        params.flags or WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                    params.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_STATE_UNCHANGED
                }

                // ── 先把权威几何推给页面，**再**改窗口 ────────────────────
                //
                // 顺序不能反。折叠时窗口从展开态（约 0.6 屏宽）缩到收起态
                // （约 1/6 屏宽），缩放系数从约 1.03 掉到约 0.29 —— 三倍多。
                // 若先改窗口，页面在之后的一两帧里仍按**旧的大系数**渲染，
                // 画布比窗口大一大截、内容被裁掉一块，几帧后才跟着缩小：
                // 用户看到的就是「折叠时闪一下」。
                //
                // 先推的话，页面在窗口变化前就拿到了新系数（前端 `liveFit`
                // 会顶着用它，见 PetMode.vue 里那段说明），窗口再收，
                // 全程只有平滑的缩小。
                //
                // 注意 [notifyMetrics] 读的是 `params.width`，此时已是**新**值。
                notifyMetrics(params, view)

                windowManager?.updateViewLayout(view, params)

                // 展开后主动请求焦点，页面里的输入框才能拿到 IME
                if (expanded) view.requestFocus()

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
                // 高度两种口径：
                // 1. logicalHeight > 0（**推荐**）：页面报的是「逻辑画布多高」，
                //    由原生按自己手里的权威窗口宽度换算成实际高度。窗口高度
                //    与窗口宽度因此在**构造上**一致——页面即使拿着过期的缩放
                //    系数，也不可能把窗口改成一个和宽度不匹配的高度。
                // 2. 否则用 height（实际 dp），只为兼容旧调用方。
                if (args.logicalHeight > 0) {
                    val logical = args.logicalHeight.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP)
                    params.height = dp((logical * currentScale(params)).coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                } else {
                    params.height = dp(args.height.coerceIn(MIN_SIZE_DP, MAX_SIZE_DP))
                }
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
     * 窗口的**权威**宽度（物理 px）—— 一律取 `params.width`。
     *
     * ## 为什么不能取 `view.width`
     *
     * `updateViewLayout()` 是**异步**的：它只把「重新布局」排进下一帧，
     * 返回时 `view.width` 仍是**上一次**布局的结果。而 show / setExpanded /
     * setSize 都在 `updateViewLayout()` 之后**立刻**把几何推给页面，
     * 于是推出去的是**上一形态**的宽度：
     *
     * ```
     * 展开时把收起态的 60dp 当成窗口宽度 → 页面算出 fit = 0.25
     * → 240dp 的画布只渲染成 60dp 宽，而窗口已经是 216dp
     * → 「展开后一大片空白，但每个框都没问题」
     * ```
     *
     * 这不是理论推演：`view.width` 在 `addView` 之后要等第一帧才有值，
     * 在 `updateViewLayout` 之后要等下一帧才更新，两条路都会踩到。
     *
     * `params.width` 是我们自己写进去的请求值，永不过期。悬浮窗带
     * `FLAG_LAYOUT_NO_LIMITS`，系统不会改写它，因此它就是真实窗口宽度。
     * 万一某个 ROM 确实改了，[notifyMetrics] 在布局落定后还会用实测值补一次。
     */
    private fun authoritativeWidthPx(params: WindowManager.LayoutParams): Int = params.width

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
    private fun currentScale(params: WindowManager.LayoutParams): Double =
        authoritativeWidthPx(params) / density.toDouble() / FLOATING_LOGICAL_WIDTH

    /**
     * 把窗口几何推给页面（`pet-metrics`）。
     *
     * 每次窗口尺寸变化后都要调：show / setExpanded / setSize。
     * 页面据此更新缩放系数，并重新回报内容高度。
     *
     * 分两拍推：
     *
     * 1. **立刻**用 [authoritativeWidthPx]（= `params.width`）推一次。
     *    这是我们自己请求的尺寸，绝不会过期，页面当场就能算出正确的缩放系数。
     * 2. **布局落定后**再用实测 `view.width` 补一次。正常情况下两者相同
     *    （`FLAG_LAYOUT_NO_LIMITS` 下系统不改窗口尺寸），所以这只是廉价的
     *    兜底；万一某 ROM 改了尺寸，页面会被拉回真实值——而第 1 拍
     *    **绝不会**推出过期值。
     *
     * 注意这只是**快路径**：原生 → 页面的事件在搬运/收回前后并不可靠，
     * 页面还会轮询 `status` 命令拿同样的数据（见 [status]）。
     */
    private fun notifyMetrics(params: WindowManager.LayoutParams, view: View?) {
        val target = view as? WebView
        pushMetrics(target, params, authoritativeWidthPx(params))
        if (target == null) return
        target.post {
            try {
                // 视图可能已被搬回 Activity（petView 易主），那就别再推了
                if (petView !== target || target.width <= 0) return@post
                pushMetrics(target, params, target.width)
            } catch (t: Throwable) {
                Log.w(TAG, "补推窗口几何失败（可忽略）", t)
            }
        }
    }

    /** 按给定宽度（物理 px）组装并派发一次 `pet-metrics`。 */
    private fun pushMetrics(view: WebView?, params: WindowManager.LayoutParams, widthPx: Int) {
        if (view == null || widthPx <= 0) return
        val widthDp = widthPx / density.toDouble()
        notifyWeb(
            view,
            "pet-metrics",
            JSObject().apply {
                put("scale", widthDp / FLOATING_LOGICAL_WIDTH)
                put("width", widthDp)
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
     *
     * 注意这里也**只置标记**，不直接唤醒：`wry` 的 `mWebView.onPause()`
     * 紧跟在 `super.onPause()` 之后，此刻唤醒会被它立刻覆盖。真正的唤醒
     * 交给 [keepAliveTick]。
     */
    override fun onPause() {
        super.onPause()
        if (!petDetached) return
        petNeedsResume = true
    }

    /** App 回到前台时同样确保 WebView 处于运行状态。 */
    override fun onResume() {
        super.onResume()
        if (!petDetached) return
        // 宿主已回到前台，wry 自己会唤醒它，不必再补
        hostResumed = true
        petNeedsResume = false
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
