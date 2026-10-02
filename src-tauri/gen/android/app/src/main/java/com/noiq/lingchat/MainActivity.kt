package com.noiq.lingchat

import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
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
    prepareWebView(webView)
  }

  /** 把 MainActivity 专属的「透壁纸」处理 + 安全区注入一次性做完。 */
  private fun prepareWebView(webView: WebView) {
    // 关键：Android WebView 默认背景是不透明的白色，必须置透明才能透出系统壁纸。
    // 前端 html/body 已设为 transparent，这里兜底去掉 WebView 的白底。
    // 硬件加速下 setBackgroundColor 即生效，无需 setLayerType(OVERLAY)（软件渲染，性能差）。
    // （注意：这一条是 MainActivity 专属 —— 它要透壁纸；设置窗不能透，见 SettingsActivity。）
    webView.setBackgroundColor(Color.TRANSPARENT)

    // 安全区 CSS 变量注入。与 SettingsActivity 共用同一个实现，
    // 见 SafeAreaInsets.kt 头部说明「为什么不能靠 env(safe-area-inset-*)」。
    SafeAreaInsets.attach(webView)
  }
}
