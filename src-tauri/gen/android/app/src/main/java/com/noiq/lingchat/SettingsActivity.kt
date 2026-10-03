package com.noiq.lingchat

import android.app.ActivityManager
import android.content.Context
import android.content.res.Configuration
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import java.lang.ref.WeakReference

/**
 * 桌宠「设置」独立窗口的宿主 Activity。
 *
 * ## 为什么需要它
 *
 * 桌面端的设置是一个**独立窗口**（`new WebviewWindow("settings", { url: "/second" })`），
 * 手机上原先照抄这段代码，但 Tauri 在 Android 上建窗的机制不同，必须显式告诉它
 * 「用哪个 Activity 承载」—— 于是这里补一个。
 *
 * 翻 `tao` 的 `platform_impl/android/ndk_glue.rs::AndroidContext::create_activity`：
 *
 * ```rust
 * let activity_class = find_class(&mut env, &main_activity,
 *         format!("{}/{activity_name}", PACKAGE.get().unwrap()))?;
 * let activity_id = jni_call_method!(env, &main_activity, "startActivity",
 *         "(Ljava/lang/Class;)I", &[(&activity_class).into()], i)?;
 * // 再等 ACTIVITY_CREATED_SENDERS 回信，超时 5 秒
 * ```
 *
 * 也就是**真正的 `startActivity(Class)`**，不是 Activity Embedding。所以只要求：
 *
 * 1. 类名要能和前端传的 `activityName` 对上，且**与 MainActivity 同包**
 *    （`PACKAGE` 取自主 Activity 的包名，写错就是 `ClassNotFoundException`）；
 * 2. 在 `AndroidManifest.xml` 里注册（`startActivity(Class)` 要求 Activity 已声明）；
 * 3. 前端建窗时传 `activityName: "SettingsActivity"`。
 *
 * ⚠️ **这条约束也意味着「一个 Tauri 窗口 = 一个 Activity」**：Android 上没法让
 * 两个 `WebviewWindow` 共用一个 Activity，也没法把设置页塞进 MainActivity 那个
 * WebView（那一个已经被桌宠占着，浮起来时还被搬进了 `WindowManager`）。
 * 所以设置页要么是现在这样「独立 Activity + 独立 WebView」，要么干脆不要独立窗口。
 * 2026-10-03 与用户确认后选的是**前者**。
 *
 * **不需要** Activity Embedding / `androidx.window` / split 规则 —— 那些是
 * 「同一屏并排显示」才要的。手机上就是新 Activity **压进返回栈**。
 *
 * ## 为什么必须有 `: TauriActivity()`
 *
 * [TauriActivity] 会跑完整的 Rust glue（`PluginManager.onActivityCreate` 等），
 * 所以这个窗口**有 Tauri IPC**：`invoke` / `emit` 照常可用。
 *
 * ⚠️ 别和「裸 `WebView(activity)`」混为一谈 —— 那个没有
 * `addJavascriptInterface`，是个读不到数据、发不出消息的空壳（见文档 4.1）。
 *
 * ## ★ 关闭语义：不销毁 + 退到后台（2026-10-03 重写）
 *
 * 早先「关闭」= `finish()`，三个后果都被真机反馈打到了：
 *
 * 1. **每次都重建**。设置页的 WebView、Pinia store、标签页选择全部重来一遍，
 *    反复调起时明显卡一下；
 * 2. **`finish()` 会落到 MainActivity** —— 也就是**聊天页**。用户报的
 *    「折叠态下可能会触发回到聊天页」就是这条：他点的是「关掉设置」，
 *    结果被扔进了另一个页面；
 * 3. 返回键（无历史时）同样走 `finish()`，症状一样。
 *
 * 现在两条路都改成 [`moveTaskToBack`]：
 *
 * | 动作        | 行为                                     | 实例 |
 * | ----------- | ---------------------------------------- | ---- |
 * | 点 ✕ / 返回 | `moveTaskToBack(true)`（整个任务退后台） | 保留 |
 * | 再点「设置」| 原生把任务拉回前台（见 [bringToFront]）  | 复用 |
 *
 * `moveTaskToBack(true)` 的参数是 `nonRoot`：**必须传 `true`** —— 传 `false`
 * 时只有「本 Activity 是任务根」才生效，而这里是任务根之上的第二个 Activity，
 * 会静默什么都不做。
 *
 * ## ★ 「推到后台就隐藏」怎么保证
 *
 * 光靠 `moveTaskToBack` 只解决了「用户主动关」这一半。另一半是：设置窗在后台
 * 活着的时候，用户从**别的入口**回到 App（桌面图标、桌宠的「返回主页」、
 * 通知……），系统会把整个任务拉到前台，**栈顶仍是设置窗** —— 用户想看聊天页，
 * 却看到设置页。用户明确要求这两种情况都必须隐藏。
 *
 * 判据是 [`pendingShow`]：
 *
 * - `onCreate` 与 [bringToFront] 会把它置真 —— 这两条是「用户主动要设置」；
 * - `onStop` 记下「确实退过后台」；
 * - `onResume` 时若**既不是主动要设置、又确实退过后台** → 自己 `finish()`。
 *
 * 用 `onStop` 而不是 `onPause`：`onPause` 会被权限弹窗、通知栏下拉这类
 * 短暂打断触发，只看它会把正常使用中的设置窗误杀。
 *
 * 走到这条分支时实例会被销毁（WebView 也就没了），但这是**用户已经离开设置
 * 语境**的情况，下次再点设置重新建一个即可 —— 而「短时间内反复调起」那条
 * 主路径（关 → 再点设置）不经过这里，实例是保留的。
 *
 * ## `configChanges` 必须与 MainActivity 对齐
 *
 * 见 `AndroidManifest.xml`：不声明这些的话，转屏会**重建 Activity** ——
 * 而 Activity 一重建，里面的 WebView 就没了，整个设置页要重新加载。
 * MainActivity 早就声明了，这里照抄。
 */
class SettingsActivity : TauriActivity() {
    /** [onWebViewCreate] 里拿到的 WebView，供转屏后强制重排用。 */
    private var settingsWebView: WebView? = null

    /** 上一次已知的 WebView 尺寸。只有真的变了才派发 resize，见 [watchSizeChanges]。 */
    private var lastWebViewW = 0
    private var lastWebViewH = 0

    /**
     * 是否**确实退过后台**（走过 `onStop`）。
     *
     * 见头注释「推到后台就隐藏」：只有退过后台再回来、且这次回来不是用户主动
     * 要设置，才认定是「从别的入口回到 App」，需要自己退场。
     * 不用 `onPause` 是为了不被权限弹窗这类短暂打断误触发。
     */
    private var wentToBackground = false

    companion object {
        private const val TAG = "SettingsActivity"

        /**
         * 当前存活的实例。
         *
         * 用 [WeakReference] 而不是强引用：Activity 归系统管，插件侧长期持强引用
         * 会把一个已经该回收的 Activity 钉在内存里（还会连带它的 WebView）。
         */
        private var instanceRef: WeakReference<SettingsActivity>? = null

        /** 取当前存活的设置窗实例；已被销毁时返回 `null`。 */
        fun current(): SettingsActivity? = instanceRef?.get()

        /**
         * 「下一次回到前台是用户主动要打开设置」。
         *
         * 由 `onCreate`（首次创建）与 [bringToFront]（复用时拉回前台）置位，
         * 在 `onResume` 里被消费掉。见头注释「推到后台就隐藏」。
         *
         * `@Volatile`：[bringToFront] 可能从 JS 桥线程进来，`onResume` 在 UI 线程读。
         */
        @Volatile
        private var pendingShow = false

        /**
         * 把存活的设置窗拉回前台。
         *
         * ## 为什么必须有这个，而不是前端 `setFocus()`
         *
         * 前端「设置」按钮发现窗口已存在时会调 `WebviewWindow.setFocus()`，
         * 但**在 Android 上那是个空实现** —— 翻
         * `tao-0.35.2/src/platform_impl/android/mod.rs`：
         *
         * ```rust
         * pub fn set_focus(&self) {
         *     warn!("set_focus not yet implemented on Android");
         * }
         * ```
         *
         * 而「关闭」现在是 `moveTaskToBack`（不 `finish()`），窗口活着但整个任务
         * 在后台。没有这条通道，用户再点「设置」就是**毫无反应** —— 窗口明明在，
         * 却永远浮不上来。
         *
         * ## 为什么用 `moveTaskToFront` 而不是 `startActivity`
         *
         * `startActivity` 那条路在这个项目上被证伪过两次（见
         * `FloatingPetPlugin.bringActivityToFront` 的注释）：MainActivity 是
         * `singleTask`，用 `startActivity` 拉它会把**它上面所有 Activity 销毁**，
         * 设置窗正好在它上面。`moveTaskToFront` 只调整任务栈顺序，不销毁任何东西。
         *
         * 需要 `REORDER_TASKS` 权限 —— `AndroidManifest.xml` 里已声明
         * （与桌宠「返回主页」共用同一条）。
         *
         * @return 是否成功发起。实例已不存在时返回 `false`，调用方应改为新建窗口。
         */
        fun bringToFront(): Boolean {
            val activity = current()
            if (activity == null) {
                Log.i(TAG, "设置窗实例已不存在，无法拉回前台")
                return false
            }
            return try {
                val am = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
                // 先置位再拉：moveTaskToFront 会同步触发 onRestart → onResume，
                // 标记必须在那一刻之前就绪，否则会被 onResume 当成「非用户主动」而自杀。
                pendingShow = true
                am.moveTaskToFront(activity.taskId, 0)
                Log.i(TAG, "设置窗已拉回前台（taskId=${activity.taskId}）")
                true
            } catch (t: Throwable) {
                // 失败要把标记退回去，否则下一次「从别的入口回 App」会误判成主动打开
                pendingShow = false
                Log.w(TAG, "把设置窗拉回前台失败", t)
                false
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // 与 MainActivity 一致：让内容铺到系统栏下面，再由前端的
        // `--safe-area-inset-*` 自己内缩。
        //
        // 注意 targetSdk = 36：Android 15+ 对 targetSdk ≥ 35 的应用**强制
        // edge-to-edge**，`themes.xml` 里的 `windowTranslucentStatus` 之类会被忽略。
        // 所以这里必须显式调用（否则旧机型上状态栏是不透明的，
        // 而新机型上是透明的，两套行为会分裂）。
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        instanceRef = WeakReference(this)
        // 首次创建也是「用户主动要设置」—— 不置位的话下面 onResume 的守卫
        // 会在第一次打开时就把设置窗关掉。
        pendingShow = true

        // 返回键：与「关闭」同一语义（退到后台），见头注释。
        //
        // 走 `OnBackPressedDispatcher` 而不是覆写 `onBackPressed()`：后者在
        // API 33+ 的预测性返回手势下**根本不会被调用**，而注册回调由 androidx
        // 自动接到 `OnBackInvokedCallback` 上。`onBackPressed()` 那份覆写保留
        // 作为兜底（Chromium 自身的返回回调在无历史时会回落到它）。
        addBackCallback()
    }

    /**
     * 注册一个「返回 = 退到后台」的回调。
     *
     * ## 为什么要注册两次
     *
     * `OnBackPressedDispatcher` 按**注册顺序倒序**派发（后注册的先被问）。
     * WebView 自己会在初始化时注册一个「能回退就 `goBack()`」的回调，而那个
     * 时点不确定 —— 若它排在后面，返回键就会被它截走（设置页没有页内历史时
     * 表现不出来，一旦有历史就变成「返回键不回退设置页」）。
     *
     * 所以 [onWebViewCreate] 里再注册一次：那个钩子必然晚于 WebView 的构造，
     * 于是我们这一份稳定排在它之后 = 稳定优先。
     *
     * 旧的那份不用移除 —— 它永远轮不到（同一时刻只有最后一个被派发），
     * 而调 `OnBackPressedCallback.remove()` 还得要求 androidx.activity ≥ 1.6，
     * 徒增版本耦合。
     */
    private fun addBackCallback() {
        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    moveTaskToBack(true)
                }
            },
        )
    }

    /**
     * 给设置窗的 WebView 注入一个「关闭本窗口」的原生接口。
     *
     * ## 为什么不能用 `Window.close()`
     *
     * 前端 `getCurrentWindow().close()` 走的是
     * `WindowMessage::Close` → `on_close_requested` → Tauri 把窗口从登记表里摘掉。
     * 但**关掉 Activity 这一步在 Android 上没人做** —— 翻 `tao` 的
     * `platform_impl/android/mod.rs`，`Window` 上**没有** `close()` / `destroy()`
     * 实现，`on_window_close` 也只是把 Rust 侧的 wrapper 置空。
     *
     * 后果：WebView 被销毁、Activity 还在 → 用户看到**一块黑屏**，
     * 只能靠返回键脱身。所以手机上关窗必须由原生来做。
     *
     * ## 为什么用 `onWebViewCreate`
     *
     * 这是 `WryActivity` 留的钩子（`open fun onWebViewCreate(webView: WebView) {}`），
     * 在 `setWebView` 里被调用，`TauriActivity` **没有**覆写它。
     *
     * 时序上也是对的：翻 `wry` 的 `android/main_pipe.rs`，`CreateWebView` 消息按
     * `activity_id` 找到本 Activity 后 `new RustWebView(activity, …)` 并调用
     * `activity.setWebView(webview)`，**之后**才 `load_url` 载入页面。
     * 所以这里一定能拿到 WebView，比 `window.decorView.post { findWebView() }`
     * 更确定（后者在 WebView 还没建出来时会直接 `return`，静默失效）。
     *
     * 接口名 `LingChatSettings` 与前端 `SettingsPage.vue` 的 `closeWindow` 对应。
     * `@JavascriptInterface` 的方法跑在 WebView 的 JavaBridge 线程上，
     * `moveTaskToBack` 必须回 UI 线程，所以套一层 `runOnUiThread`。
     */
    override fun onWebViewCreate(webView: WebView) {
        settingsWebView = webView

        // 再挂一份返回键回调，让它稳定排在 WebView 内建回调之前（见 addBackCallback）
        addBackCallback()

        // 安全区：设置窗也是 edge-to-edge，不注入就会被状态栏 / 手势条压住。
        // 变量由前端 `.pt-safe / .pb-safe / .pl-safe / .pr-safe` 消费
        // （见 SettingsPage.vue 的根元素）。
        SafeAreaInsets.attach(webView)

        // 尺寸真的变了就派发 resize（后台转屏、分屏等 onConfigurationChanged 漏掉的场景）
        watchSizeChanges(webView)

        webView.addJavascriptInterface(
            object {
                @JavascriptInterface
                fun close() {
                    runOnUiThread { moveTaskToBack(true) }
                }
            },
            "LingChatSettings",
        )
    }

    /**
     * 监视 WebView 的**实际尺寸**，变了才给页面派发一次 `resize`。
     *
     * 与 [MainActivity] 同一套理由：`onConfigurationChanged` 只在配置变化那一刻
     * 触发，而 Activity 在后台转屏时那一刻窗口尺寸还没变（要等回到前台才真正测量），
     * 于是页面按旧尺寸排版。真机表现就是「在别的 App 里转屏再切回来，
     * 设置页还是按旧方向排版、右边多出一截空白」。
     *
     * 监听 `OnLayoutChangeListener` 是按**事实**触发，同时覆盖后台转屏、分屏拖拽、
     * 折叠屏展开收起、键盘压小内容区这几种情况。
     *
     * 用尺寸去重：这个回调每次布局都会来一遍，不去重会被 resize 风暴打爆
     * （前端的布局计算都挂在 resize 上）。
     */
    private fun watchSizeChanges(webView: WebView) {
        webView.addOnLayoutChangeListener { v, left, top, right, bottom, _, _, _, _ ->
            val w = right - left
            val h = bottom - top
            if (w == lastWebViewW && h == lastWebViewH) return@addOnLayoutChangeListener
            lastWebViewW = w
            lastWebViewH = h
            Log.i(TAG, "WebView 尺寸变为 ${w}x${h}，派发 resize")
            v.post {
                val view = v as? WebView ?: return@post
                // 尺寸变了往往也意味着 insets 变了（转屏），顺手刷一次安全区
                SafeAreaInsets.reapply(view)
                view.evaluateJavascript("window.dispatchEvent(new Event('resize'));", null)
            }
        }
    }

    /** 记下「确实退过后台」，供 [onResume] 的守卫判断。见头注释。 */
    override fun onStop() {
        super.onStop()
        wentToBackground = true
    }

    /**
     * 回到前台时决定：正常显示，还是自己退场。
     *
     * 见头注释「推到后台就隐藏」。三种情况：
     *
     * 1. 首次创建后第一次 resume → [pendingShow] 已由 `onCreate` 置位，正常显示；
     * 2. 用户点「设置」把它拉回来 → [pendingShow] 由 [bringToFront] 置位，正常显示；
     * 3. 用户从桌面图标 / 桌宠「返回主页」等**别的入口**回到 App
     *    → 没人置位，且 `wentToBackground` 为真 → `finish()`，把栈顶让给
     *    MainActivity（否则用户想看聊天页却看到设置页）。
     *
     * `finish()` 用 post 而不是直接调：`onResume` 里同步结束自己会踩到
     * 生命周期回调的重入，post 到下一帧最稳。
     */
    override fun onResume() {
        super.onResume()

        val requested = pendingShow
        pendingShow = false
        if (requested) {
            // 用户主动要的，正常显示；顺手清掉「退过后台」的痕迹
            wentToBackground = false
            return
        }

        if (wentToBackground) {
            wentToBackground = false
            Log.i(TAG, "非用户主动回到前台，设置窗自行退场")
            window.decorView.post { finish() }
        }
    }

    override fun onDestroy() {
        // 只清掉指向自己的那份引用；新的实例会在自己的 onCreate 里重新登记
        if (instanceRef?.get() === this) instanceRef = null
        super.onDestroy()
    }

    /**
     * 返回键兜底。
     *
     * 主路径是 `onCreate` 里注册的 [OnBackPressedCallback]。这条是给
     * 「Chromium 自己的返回回调先拿到事件、发现 `canGoBack()` 为假、于是
     * `isEnabled = false` 再回落到 `onBackPressed()`」准备的
     * （见文档 4.6.1）。
     */
    @Deprecated("覆写只为兜底，主路径是 OnBackPressedDispatcher")
    override fun onBackPressed() {
        moveTaskToBack(true)
    }

    /**
     * 转屏（以及分屏、折叠屏展开等）后的强制重排。
     *
     * ## 为什么必须自己写
     *
     * `AndroidManifest.xml` 里给本 Activity 声明了
     * `configChanges="orientation|screenSize|…"`，好处是转屏**不重建** Activity
     * （WebView 不会被销毁、设置页不用重新加载），代价是**系统不再帮我们重排** ——
     * 它只调一次 `onConfigurationChanged`，剩下的全归应用自己。
     * 而 `TauriActivity` 只把它转发给 `PluginManager`（插件默认空实现）。
     *
     * 于是：Activity 在后台时转屏（用户在别的 App 里转了屏再切回来）最明显 ——
     * 窗口尺寸是回到前台时才真正变的，WebView 的视觉视口
     * （`window.innerWidth/innerHeight`）可能仍停在**旧方向**，页面就按旧尺寸
     * 排版；横屏回竖屏时表现为「内容比屏幕宽、右边被切掉」。
     *
     * 这里补三件事，缺一不可：
     * 1. `requestApplyInsets()`：重新分发 insets，刷新安全区 CSS 变量
     *    （横竖屏的 `--safe-area-inset-*` 是不同的，挖孔会换到另一侧）；
     * 2. `requestLayout()`：强制 WebView 重新测量；
     * 3. 给页面派发一次 `resize`：前端的布局（`--pet-fit`、画布高度、
     *    各种尺寸缓存）都挂在 resize 上，光重测 DOM 不会让 JS 重算。
     */
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)

        window.decorView.requestApplyInsets()

        settingsWebView?.let { webView ->
            webView.requestLayout()
            webView.evaluateJavascript(
                "window.dispatchEvent(new Event('resize'));",
                null,
            )
        }
    }
}
