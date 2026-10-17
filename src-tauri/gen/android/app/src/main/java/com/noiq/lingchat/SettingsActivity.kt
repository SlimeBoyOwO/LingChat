package com.noiq.lingchat

import android.app.ActivityManager
import android.content.Context
import android.content.Intent
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
 * ## ★ 关闭语义：退回 + 不销毁 + 独占任务栈（2026-10-03 重写）
 *
 * 早先「关闭」= `finish()`，三个后果都被真机反馈打到了：
 *
 * 1. **每次都重建**。设置页的 WebView、Pinia store、标签页选择全部重来一遍，
 *    反复调起时明显卡一下；
 * 2. **`finish()` 会落到 MainActivity** —— 也就是**聊天页**。用户报的
 *    「关掉设置却回到聊天页」就是这条：他点的是「关掉设置」，
 *    结果被扔进了另一个页面；
 * 3. 返回键（无历史时）同样走 `finish()`，症状一样。
 *
 * 现在**任何一条路都不再 `finish()`**，实例与它的 WebView 全程保留：
 *
 * | 动作                        | 行为                                             | 实例 |
 * | --------------------------- | ------------------------------------------------ | ---- |
 * | 点 ✕ / 返回键               | [sendWholeAppToBack]：设置任务 + 聊天任务一起退后台 | 保留 |
 * | 桌宠的「返回主页 / 返回键」 | 原生 `moveTaskToFront(聊天任务)`，设置任务留在后面 | 保留 |
 * | 再点「设置」                | 原生把设置任务拉回前台（见 [bringToFront]）        | 复用 |
 *
 * ### 为什么必须独占一个任务栈
 *
 * 本 Activity 在 `AndroidManifest.xml` 里声明了
 * `taskAffinity` + `launchMode="singleInstance"` + `excludeFromRecents`。
 * 那不是风格选择，是**上面表格第二行能成立的前提**：
 *
 * 设置窗原先与 MainActivity 同一个任务栈，于是「按桌宠返回键要回到聊天页」
 * 这件事根本无解 —— `moveTaskToBack` 是**任务级**的（会把聊天页一起退到后台，
 * 用户看到的是桌面），而 `FloatingPetPlugin.bringActivityToFront()` 的
 * `moveTaskToFront(MainActivity.taskId)` 拉的是**整个任务**、栈顶仍是设置窗
 * （**真机反馈的正是这条**：「进入桌宠设置后，直接按桌宠上的返回键，
 * 设置页没有收起来」）。
 *
 * 让设置窗独占任务栈之后，`moveTaskToFront(聊天任务)` 只把聊天页翻到最前面，
 * 设置窗原样留在它自己的任务里 —— 退回、且不销毁。
 *
 * ### `moveTaskToBack` 的参数
 *
 * 参数是 `nonRoot`：**必须传 `true`**。传 `false` 时只有「本 Activity 是
 * 任务根」才生效；`true` 表示「任务里任意一个 Activity 都能把整个任务退到
 * 后台」。（本 Activity 现在恰好是设置任务的根，但传 `true` 才是不依赖这个
 * 前提的写法。）
 *
 * ### ✕ / 返回键为什么要连聊天任务一起退
 *
 * 独占任务栈之后，`moveTaskToBack(true)` 只退设置任务，底下的聊天任务会顺势
 * 浮到前台 —— 用户点「关掉设置」看到的会是聊天页。而他要的是「回到进来之前
 * 那个 App」（第 2 条真机反馈说的就是这个），所以两个任务都要退。顺序也有讲究，
 * 见 [sendWholeAppToBack]。
 *
 * ## ★ 「推到后台就自己 finish()」的守卫已**删除**（2026-10-03，真机 ANR 后）
 *
 * 原先 `onResume` 里有一条兜底：若这次回到前台**不是**用户主动要设置、
 * 且确实退过后台，就自己 `finish()`。它已经整个拆掉，原因有两条：
 *
 * ### 1. 它是「Tauri 窗口登记表」与「Android 实例」失同步的**唯一来源**
 *
 * 前端的 `handleOpenSettings` 要靠 `LingChatApp.focusSettings()` 判断
 * 「实例还在不在」（见 `MainActivity.installSettingsBridge`），而这个判据读的
 * 就是本类 `companion object` 里的 `instanceRef`。
 *
 * 一旦这里 `finish()`，`instanceRef` 变 null，但 **Tauri 那边的窗口登记还在**
 * —— 于是前端会以为「实例没了」，掉进新建分支再发一次建窗。而 `launchMode` 是
 * `singleInstance`：系统**复用**已有实例走 `onNewIntent`，`onCreate` 不再触发，
 * `tao` 的 `create_activity` 就永远等不到回信 —— 那条路上还要拿着 Tauri 的
 * `webviews_lock()`，会把主线程一起拖住。**这就是真机 5 秒 ANR 的成因之一。**
 *
 * ### 2. 它跟「设置页只用退回、不要关闭」的既定方向冲突
 *
 * 用户明确要求设置页**退回而不销毁**（省掉 WebView / store 的重建开销）。
 * 一个会在某些入口下偷偷 `finish()` 的守卫，正好是这个方向的漏洞。
 *
 * ### 那「从别的入口回 App 看到设置页」怎么办
 *
 * 不靠守卫，靠**任务栈隔离**：本 Activity 声明了 `taskAffinity` +
 * `singleInstance` + `excludeFromRecents`，桌面图标 / 桌宠返回键拉起的都是
 * **聊天任务**，设置任务只会留在后面、不会被 resume —— 本来就不会出现
 * 「想看聊天页却看到设置页」。守卫是当年同栈时代的补丁，现在没有存在必要。
 *
 * ⚠️ 现在的语义是：**本 Activity 一旦创建就活到进程结束**（或被系统回收）。
 * 所有「关闭」都是 `moveTaskToBack`，不销毁。见上面的「关闭语义」表。
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
         * 而「关闭」现在既不 `finish()` 也不销毁 —— 窗口活着，只是它所在的那个
         * 设置任务退到了后台（见头注释「关闭语义」）。没有这条通道，用户再点
         * 「设置」就是**毫无反应**：窗口明明在，却永远浮不上来。
         *
         * ## 为什么用 `moveTaskToFront` 而不是 `startActivity`
         *
         * `startActivity` 那条路在这个项目上被证伪过两次（见
         * `FloatingPetPlugin.bringActivityToFront` 的注释）：MainActivity 是
         * `singleTask`，用 `startActivity` 拉它会把**它上面所有 Activity 销毁**。
         * `moveTaskToFront` 只调整任务栈顺序，不销毁任何东西 —— 而且设置窗现在
         * 有自己的任务栈，拉它本来也只该走这条路。
         *
         * 需要 `REORDER_TASKS` 权限 —— `AndroidManifest.xml` 里已声明
         * （与桌宠「返回主页」共用同一条）。
         *
         * @return 是否成功发起。实例已不存在时返回 `false`。
         *
         * ⚠️ 调用方**只有在拿到 `false` 时**才允许新建窗口，而且新建那一步必须
         * 防重入（见 `PetMode.vue` 的 `settingsOpening`）：连续两次建窗会踩到
         * Tauri `webviews_lock()` 与页面加载回调的互锁 —— 真机上表现为
         * 「点桌宠没反应」，5 秒后 ANR。
         */
        fun bringToFront(): Boolean {
            val activity = current()
            if (activity == null) {
                Log.i(TAG, "设置窗实例已不存在，无法拉回前台")
                return false
            }
            return try {
                val am = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
                am.moveTaskToFront(activity.taskId, 0)
                Log.i(TAG, "设置窗已拉回前台（taskId=${activity.taskId}）")
                true
            } catch (t: Throwable) {
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

        // 返回键：与「关闭」同一语义（退到后台），见头注释。
        //
        // 走 `OnBackPressedDispatcher` 而不是覆写 `onBackPressed()`：后者在
        // API 33+ 的预测性返回手势下**根本不会被调用**，而注册回调由 androidx
        // 自动接到 `OnBackInvokedCallback` 上。`onBackPressed()` 那份覆写保留
        // 作为兜底（Chromium 自身的返回回调在无历史时会回落到它）。
        addBackCallback()
    }

    /**
     * 实例被复用时的回调（`singleTask` 下再次 `startActivity` 走到这里）。
     *
     * ## 为什么需要它
     *
     * `singleTask` 复用已有实例时**不会**再走 `onCreate`，所以「每次被拉到前台
     * 都该做一次」的事必须在这里补一遍，否则第二次打开设置页会带着上一次的
     * insets / 布局状态。
     *
     * ## ⚠️ 前端**不应该**走到这里
     *
     * 按 manifest 里的硬约束，`PetMode.handleOpenSettings` 会先问原生
     * `bringToFront()`，实例还在就 `moveTaskToFront` 拉回、**不再调 tao 建窗** ——
     * 因为 tao 建窗后会等 `ACTIVITY_CREATED` 回调，而复用实例根本不触发它，
     * 那次建窗会白等 5 秒然后失败。
     *
     * 这个覆写是**防御性**的：万一某条路径真的又发了 `startActivity`
     * （比如用户在最近任务里点回设置卡片），至少让实例正确回到前台并重排，
     * 而不是停在一个尺寸已经过期的画面上。
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        Log.i(TAG, "设置窗实例被复用（onNewIntent，taskId=$taskId）")
        // 复用后视口可能与上次不同（分屏、折叠屏展开、转屏后切回），补一次重排。
        window.decorView.requestApplyInsets()
        settingsWebView?.let { webView ->
            webView.requestLayout()
            webView.evaluateJavascript(
                "window.dispatchEvent(new Event('resize'));",
                null,
            )
        }
    }

    /**
     * 收起设置页：把**整个 App**（设置任务 + 聊天任务）退到后台。
     *
     * ## 为什么是「两个任务」而不是自己一个
     *
     * 设置窗从 2026-10-03 起独占一个任务栈（见头注释「关闭语义」与
     * `AndroidManifest.xml`），所以 `moveTaskToBack(true)` **只退设置任务** ——
     * 底下的聊天任务会顺势浮到前台，用户点「关掉设置」看到的会是**聊天页**。
     *
     * 而他要的是「回到进来之前那个 App」：这条真机反馈过（`finish()` 时代
     * 「关掉设置却回到聊天页」），也正是 ✕ / 返回键一直以来的语义。
     * 所以这里要把聊天任务也退掉。
     *
     * ## 顺序不能反
     *
     * **先**退聊天任务、**再**退自己：
     *
     * - 按这个顺序，第一步执行时设置页还在最前面，屏幕上看不出任何变化；
     *   第二步才把设置页收走，直接露出**进来之前那个 App**。
     * - 反过来的话，设置任务先退、聊天任务会短暂成为前台 —— 用户会瞥见一眼
     *   聊天页，然后才被退到后台。
     *
     * `MainActivity.sendTaskToBack()` 是同步的（它内部用 `runOnUiThread`，
     * 而当前本来就在主线程上，所以是直接内联执行），两步之间的顺序稳定成立。
     *
     * 拿不到 MainActivity 实例时（理论上只在进程重启的瞬间）只退自己 ——
     * 退化成「露出聊天页」，不是崩，可接受。
     */
    private fun sendWholeAppToBack() {
        MainActivity.sendTaskToBack()
        moveTaskToBack(true)
    }

    /**
     * 注册一个「返回 = 收起设置页」的回调。
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
                    sendWholeAppToBack()
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
     * `@JavascriptInterface` 的方法跑在 WebView 的 JavaBridge 线程上，而
     * [sendWholeAppToBack] 里两步任务栈操作都该在主线程上做，所以套一层
     * `runOnUiThread`。
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
                    runOnUiThread { sendWholeAppToBack() }
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

    // ─── `onStop` / `onResume` 的「退到后台就自己 finish()」守卫已删除 ───
    //
    // 原先是 `onStop` 记 `wentToBackground = true`，`onResume` 里若这次回来不是
    // 用户主动要设置、又确实退过后台，就 `window.decorView.post { finish() }`。
    //
    // 整个拆掉了，理由见头注释「推到后台就自己 finish() 的守卫已删除」：
    // 它是「Tauri 窗口登记表」与「Android 实例」失同步的唯一来源，会把前端推进
    // 那条「实例没了 → 再建一次窗」的路，而那条路在 Android 上会死锁（5s ANR）；
    // 而且它跟「设置页只用退回、不要关闭」的既定方向冲突。
    //
    // 现在本 Activity 没有覆写 `onStop` / `onResume` —— 回到前台就正常显示，
    // 不存在「自己退场」这条分支。

    override fun onDestroy() {
        // ★ 诊断埋点：「设置窗被谁销毁」是 2026-10-03 那轮 ANR 里唯一没有定论
        // 的一环。真机上观察到它被连续建了四次（t1900 → t1901 → t1902 → t1903），
        // 每次 2~4 秒后就消失，而本类与 `MainActivity.sendTaskToBack()` 里
        // **都没有** `finish()`（全走 `moveTaskToBack`）。
        //
        // 这三个标记能区分三种来源：
        //   · isFinishing = true       → 有人明确调了 finish()（tao 的 close，或系统）
        //   · isChangingConfigurations → 配置变更导致的正常重建（不该发生，
        //                                manifest 里已声明 configChanges）
        //   · 两者都 false             → 系统直接回收（内存 / task 清理）
        Log.i(
            TAG,
            "onDestroy：isFinishing=$isFinishing " +
                "isChangingConfigurations=$isChangingConfigurations " +
                "taskId=$taskId",
        )
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
        sendWholeAppToBack()
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
