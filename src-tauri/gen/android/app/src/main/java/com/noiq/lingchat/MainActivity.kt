package com.noiq.lingchat

import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.util.Log
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import java.lang.ref.WeakReference

private const val TAG = "LingChatMain"

class MainActivity : TauriActivity() {
  /** [onWebViewCreate] 里拿到的 WebView，供转屏后强制重排用。 */
  private var mainWebView: WebView? = null

  /** 已经挂过尺寸监听的 WebView，避免 `prepareWebView` 被两条路径调用时重复挂。 */
  private var watchedWebView: WebView? = null

  /** 上一次已知的 WebView 尺寸。只有真的变了才派发 resize，见 [watchSizeChanges]。 */
  private var lastWebViewW = 0
  private var lastWebViewH = 0

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    instanceRef = WeakReference(this)

    // 透明窗口背景，配合 themes.xml 的 windowShowWallpaper 透出系统壁纸
    // （WebView 本身仍需单独置透明，见 prepareWebView）
    window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))

    // 返回键：见 installBackCallback 的说明。这里这一份只是兜底
    // （`onCreate` 早于 WebView 构造，可能排在 Chromium 那份**前面**而轮不到），
    // 真正稳定生效的是 `onWebViewCreate` 里那一份。
    installBackCallback()

    // 兜底路径：见 onWebViewCreate 的说明。这里找不到 WebView 就什么都不做。
    window.decorView.post {
      SafeAreaInsets.findWebView(window.decorView as ViewGroup)?.let(::prepareWebView)
    }
  }

  override fun onDestroy() {
    // 只清掉指向自己的那份引用；重建后的新实例会在自己的 onCreate 里重新登记
    if (instanceRef?.get() === this) instanceRef = null
    super.onDestroy()
  }

  /**
   * 主路径：`WryActivity.setWebView` 的回调，**确定**拿得到 WebView。
   *
   * ## 为什么要两条路径
   *
   * 原来只有 `onCreate` 里的 `window.decorView.post { … }`，但那是**赌**：
   * `setWebView` 由 Rust 主线程在窗口创建时调过来（见 `wry` 的
   * `android/main_pipe.rs`：`CreateWebView` → `new RustWebView(activity, …)` →
   * `activity.setWebView(webview)`），和 `decorView.post` 的下一帧谁先谁后
   * 并不确定。`findWebView` 扑空时是 `?: return` —— **静默失效**，
   * 安全区变量永远不注入，前端只能回退到 `env()`（Android WebView 上恒为 0）。
   *
   * 这个钩子在 `setWebView` 里被调用，且**早于** `load_url`，所以必定命中。
   * 两条路径都是幂等的（`setBackgroundColor` 幂等；`SafeAreaInsets.attach`
   * 是替换监听器），重复执行无害。
   */
  override fun onWebViewCreate(webView: WebView) {
    mainWebView = webView
    // ★ 必须在这里再注册一次：`onBackPressedDispatcher` 按注册顺序**倒序**派发，
    // 而 Chromium 自带的返回回调是在 WebView 构造时注册的 —— 本钩子必然晚于它，
    // 于是我们这一份稳定排在最后 = 稳定优先。详见 installBackCallback。
    installBackCallback()
    prepareWebView(webView)
  }

  /**
   * 转屏（以及分屏、折叠屏展开等）后的强制重排。
   *
   * 与 `SettingsActivity` 同一套理由：`configChanges` 里声明了
   * `orientation|screenSize|…`，系统因此**不会**重建 Activity，也就**不会**
   * 帮我们重排 —— 它只调一次 `onConfigurationChanged`，而 `TauriActivity`
   * 仅把它转发给 `PluginManager`（插件默认空实现）。
   *
   * 真机症状：在**别的 App 里**转屏再切回来，主界面按旧方向排版（内容比屏幕
   * 宽 / 高，一侧被切掉）；而在本 App 内转屏是好的 —— 差别就在于后台转屏时
   * 窗口尺寸是回到前台那一刻才真正变的，WebView 的视觉视口没跟着更新。
   *
   * ⚠️ 桌宠在悬浮窗里时这个 WebView 已被原生搬进 `WindowManager`，
   * 不在本 Activity 的视图树里。那种情况下几何由 `FloatingPetPlugin`
   * 自己的 `DisplayManager` 回调 + 保活轮询负责；这里多派发一次 resize
   * 只会让页面用当前视口重算一次，无害。
   */
  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)

    // 重新分发 insets：横竖屏的挖孔位置不同，安全区 CSS 变量必须跟着换边
    window.decorView.requestApplyInsets()

    mainWebView?.let { webView ->
      webView.requestLayout()
      webView.evaluateJavascript(
        "window.dispatchEvent(new Event('resize'));",
        null,
      )
    }
  }

  /** 把 MainActivity 专属的「透壁纸」处理 + 安全区注入 + JS 桥一次性做完。 */
  private fun prepareWebView(webView: WebView) {
    // 关键：Android WebView 默认背景是不透明的白色，必须置透明才能透出系统壁纸。
    // 前端 html/body 已设为 transparent，这里兜底去掉 WebView 的白底。
    // 硬件加速下 setBackgroundColor 即生效，无需 setLayerType(OVERLAY)（软件渲染，性能差）。
    // （注意：这一条是 MainActivity 专属 —— 它要透壁纸；设置窗不能透，见 SettingsActivity。）
    webView.setBackgroundColor(Color.TRANSPARENT)

    // 安全区 CSS 变量注入。与 SettingsActivity 共用同一个实现，
    // 见 SafeAreaInsets.kt 头部说明「为什么不能靠 env(safe-area-inset-*)」。
    SafeAreaInsets.attach(webView)

    // 与 SettingsActivity 通信的 JS 桥（详见下方 installSettingsBridge）。
    installSettingsBridge(webView)

    // 尺寸真的变了就派发 resize。见 watchSizeChanges 的说明 ——
    // 这是 onConfigurationChanged 之外的**兜底**，覆盖它在后台转屏时漏掉的场景。
    watchSizeChanges(webView)
  }

  /**
   * 监视 WebView 的**实际尺寸**，变了才给页面派发一次 `resize`。
   *
   * ## 为什么 `onConfigurationChanged` 还不够
   *
   * [onConfigurationChanged] 只在**配置变化那一刻**被调用。而 Activity 在后台时
   * 转屏，那一刻窗口尺寸**还没变**（要等回到前台才真正测量）—— 此时读到的
   * `window.innerWidth/innerHeight` 仍是旧方向的值，页面于是按旧尺寸排版；
   * 真机表现就是「在别的 App 里转屏再切回来，主界面还是横/竖排错乱」。
   *
   * 监听 `OnLayoutChangeListener` 是按**事实**触发：WebView 什么时候真的换了尺寸，
   * 就什么时候派发。它同时覆盖：
   *
   * - 后台转屏后回到前台；
   * - 分屏 / 自由窗口拖拽改变大小；
   * - 折叠屏展开收起；
   * - 键盘弹出把内容区压小（`ADJUST_RESIZE`）。
   *
   * 用「尺寸是否真的变了」做去重：这个回调在每次布局都会来一遍，
   * 不去重的话页面会被 resize 风暴打爆（前端 `--pet-fit` 等都在 resize 上重算）。
   *
   * ⚠️ 桌宠在悬浮窗里时这个 WebView 已被原生搬进 `WindowManager`，
   * 不在本 Activity 的视图树里，本监听不会触发 —— 那种情况由
   * `FloatingPetPlugin` 自己的 `DisplayManager` 回调负责。
   */
  private fun watchSizeChanges(webView: WebView) {
    if (watchedWebView === webView) return
    watchedWebView = webView
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

  /**
   * 在主窗口的 WebView 上挂一个 `LingChatApp` 桥，只暴露一件事：
   * 「把已经存在但退到后台的设置窗拉回前台」。
   *
   * ## 为什么必须有它
   *
   * `SettingsActivity` 的「关闭」语义是**退回、不销毁**：它独占一个任务栈
   * （见 `AndroidManifest.xml`），关掉时整个设置任务退到后台，Activity 实例
   * 一直活着。用户再点「设置」时，前端走的是
   * `WebviewWindow.getByLabel("settings")` → `setFocus()`。
   *
   * 但 `setFocus()` **在 Android 上是空实现** ——
   * `tao-0.35.2/src/platform_impl/android/mod.rs` 里只有一句
   * `warn!("set_focus not yet implemented on Android")`。
   * 没有这条桥，用户第二次点「设置」就是**毫无反应**。
   *
   * ## 为什么是 JS 桥而不是 Tauri 命令
   *
   * 拉回前台这件事只有**主窗口**的 JS 需要（判断窗口是否存在、再决定是新建
   * 还是拉回）。而 `SettingsActivity` 在 app 模块里，`tauri-plugin-floating-pet`
   * 是它依赖的 library 模块 —— 插件反向引用 app 的类得靠反射。JS 桥同模块直调，
   * 少一层且编译期就能查错。
   *
   * ## 幂等
   *
   * `addJavascriptInterface` 对同名接口是**替换**语义，而 [prepareWebView] 会被
   * 两条路径各调一次（`onWebViewCreate` 主路径 + `onCreate` 里的兜底）。
   * 重复挂同一个名字只会把旧对象换掉，无害。
   *
   * ⚠️ `@JavascriptInterface` 的方法跑在 WebView 的 JavaBridge 线程上，
   * 里面不能碰 UI（`moveTaskToFront` 是 binder 调用，本身线程安全）。
   * R8 对带 `@JavascriptInterface` 的方法有默认 keep 规则，不会被改名。
   */
  private fun installSettingsBridge(webView: WebView) {
    webView.addJavascriptInterface(
      object {
        /** @return 设置窗是否还活着（活着才会被拉回前台）。 */
        @JavascriptInterface
        fun focusSettings(): Boolean = SettingsActivity.bringToFront()
      },
      "LingChatApp",
    )
  }

  /**
   * 返回键：**退到后台**（不销毁）。
   *
   * ## 为什么必须自己注册
   *
   * `TauriActivity` 把 `handleBackNavigation` 覆写成了 `false`，于是
   * `WryActivity.setWebView` **不注册**它那个「能回退就 `goBack()`」的回调。
   * 但 **Chromium 的 `WebView` 自己会注册一个** —— 返回键于是被它截走，
   * 变成「主 WebView 的历史回退」。
   *
   * 真机症状（用户原话）：「软件在前台主活动，按返回键会导致桌宠里面被返回」。
   * 原因就是主 WebView 的历史里通常留着 `/pet`：从桌宠回到聊天页之后按返回键，
   * `goBack()` 直接**退回了桌宠页**。
   *
   * ## 为什么注册两次
   *
   * `OnBackPressedDispatcher` 按**注册顺序倒序**派发（后注册的先被问），而
   * Chromium 那份是在 WebView 构造时注册的。`onCreate` 里那一份可能排在它
   * **前面**（永远轮不到），[onWebViewCreate] 里那一份才稳定生效。
   * 两份行为完全一致，旧的那份不用移除 —— 同一时刻只有最后一个会被派发。
   *
   * ## 为什么是 `moveTaskToBack` 而不是 `finish()`
   *
   * 与设置页同一套理由（见 `SettingsActivity` 头注释「关闭语义」）：用户明确
   * 要求「尽量只用退回，不要用关闭，以节省性能」。退到后台后 WebView、Pinia
   * store、桌宠状态全部保留，再回来是原样。
   *
   * ## ⚠️ 代价：页面内的「返回上一级」不再由返回键承担
   *
   * 注册之后返回键**不再**走 `goBack()`，也就是「聊天页 → 主菜单」这类页内
   * 后退不能再靠返回键完成（改由页面上的按钮承担）。这是刻意的取舍 ——
   * 「根页面按返回键应该退出 App」比「浏览器式后退」更符合用户预期。
   * 将来若要两者兼得，需要前端上报「当前是不是根路由」，原生据此分流。
   */
  private fun installBackCallback() {
    onBackPressedDispatcher.addCallback(
      this,
      object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
          moveTaskToBack(true)
        }
      },
    )
  }

  companion object {
    /**
     * 当前存活的主界面实例。
     *
     * 用 [WeakReference] 而不是强引用：Activity 归系统管，长期持强引用会把一个
     * 已经该回收的 Activity（连同它的 WebView）钉在内存里。
     *
     * 只在 `onCreate` / `onDestroy` 里写，读它的只有 [sendTaskToBack]。
     */
    private var instanceRef: WeakReference<MainActivity>? = null

    /**
     * 把**聊天页所在的任务**退到后台。
     *
     * ## 谁需要它
     *
     * 只有 `SettingsActivity` 的 ✕ / 返回键（见 `sendWholeAppToBack`）。
     * 设置窗从 2026-10-03 起独占一个任务栈（见 `AndroidManifest.xml`），
     * 于是它自己的 `moveTaskToBack(true)` **只退设置任务**，底下的聊天任务
     * 会顺势浮到前台 —— 用户点「关掉设置」看到的会是**聊天页**。
     * 而他要的是「回到进来之前那个 App」（`finish()` 时代真机反馈过这条），
     * 所以必须把聊天任务也退掉。
     *
     * ## 为什么是静态方法而不是 JS 桥
     *
     * 调用方是同进程的另一个 Activity，直调即可；走 JS 桥反而要绕一圈
     * 且得先拿到主窗口的 WebView 引用。
     *
     * ## 线程
     *
     * 调用方必须在主线程上调。这里用 `runOnUiThread` 保证 `moveTaskToBack`
     * 落在主线程，同时 —— 因为 `runOnUiThread` 在「当前就是主线程」时会**直接
     * 内联执行** —— 让「先退聊天任务、再退设置任务」的顺序同步成立
     * （见 `SettingsActivity.sendWholeAppToBack` 对顺序的说明）。
     *
     * @return 是否确实发起（主界面实例已被回收时返回 `false`）。
     */
    fun sendTaskToBack(): Boolean {
      val activity = instanceRef?.get() ?: return false
      activity.runOnUiThread { activity.moveTaskToBack(true) }
      return true
    }
  }
}
