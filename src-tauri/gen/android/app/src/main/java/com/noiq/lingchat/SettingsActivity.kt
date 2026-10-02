package com.noiq.lingchat

import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

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
 * **不需要** Activity Embedding / `androidx.window` / split 规则 —— 那些是
 * 「同一屏并排显示」才要的。手机上就是新 Activity **压进返回栈**，
 * 按返回回到桌宠所在的 MainActivity。
 *
 * ## 为什么必须有 `: TauriActivity()`
 *
 * [TauriActivity] 会跑完整的 Rust glue（`PluginManager.onActivityCreate` 等），
 * 所以这个窗口**有 Tauri IPC**：`invoke` / `emit` 照常可用。
 *
 * ⚠️ 别和「裸 `WebView(activity)`」混为一谈 —— 那个没有
 * `addJavascriptInterface`，是个读不到数据、发不出消息的空壳（见文档 4.1）。
 *
 * ## 为什么不覆写 `handleBackNavigation`
 *
 * 保持 [TauriActivity] 的默认值 `false` 即可：返回键交给 WebView 自身处理，
 * 设置页没有可回退的历史时就落到 Activity 的 `onBackPressed()` → `finish()`，
 * 正好就是「关掉设置窗、回到桌宠」。这与文档 4.6 描述的返回键行为一致。
 *
 * ## `configChanges` 必须与 MainActivity 对齐
 *
 * 见 `AndroidManifest.xml`：不声明这些的话，转屏会**重建 Activity** ——
 * 而 Activity 一重建，里面的 WebView 就没了，整个设置页要重新加载。
 * MainActivity 早就声明了，这里照抄。
 */
class SettingsActivity : TauriActivity() {
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
     * 只能靠返回键脱身。所以手机上关窗必须由原生 `finish()` 来做。
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
     * `finish()` 必须回 UI 线程，所以套一层 `runOnUiThread`。
     */
    override fun onWebViewCreate(webView: WebView) {
        // 安全区：设置窗也是 edge-to-edge，不注入就会被状态栏 / 手势条压住。
        // 变量由前端 `.pt-safe / .pb-safe / .pl-safe / .pr-safe` 消费
        // （见 SettingsPage.vue 的根元素）。
        SafeAreaInsets.attach(webView)

        webView.addJavascriptInterface(
            object {
                @JavascriptInterface
                fun close() {
                    runOnUiThread { finish() }
                }
            },
            "LingChatSettings",
        )
    }
}
