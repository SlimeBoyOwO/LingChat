package com.noiq.lingchat

import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  /** [onWebViewCreate] 里拿到的 WebView，供转屏后强制重排用。 */
  private var mainWebView: WebView? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // 透明窗口背景，配合 themes.xml 的 windowShowWallpaper 透出系统壁纸
    // （WebView 本身仍需单独置透明，见 prepareWebView）
    window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))

    // 兜底路径：见 onWebViewCreate 的说明。这里找不到 WebView 就什么都不做。
    window.decorView.post {
      SafeAreaInsets.findWebView(window.decorView as ViewGroup)?.let(::prepareWebView)
    }
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
  }

  /**
   * 在主窗口的 WebView 上挂一个 `LingChatApp` 桥，只暴露一件事：
   * 「把已经存在但退到后台的设置窗拉回前台」。
   *
   * ## 为什么必须有它
   *
   * `SettingsActivity` 的「关闭」语义是**不销毁 + 退到后台**（`moveTaskToBack`），
   * 所以窗口会一直活着。用户再点「设置」时，前端走的是
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
}
