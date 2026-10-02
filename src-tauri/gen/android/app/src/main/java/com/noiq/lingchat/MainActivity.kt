package com.noiq.lingchat

import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsCompat.Type.systemBars

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // 透明窗口背景，配合 themes.xml 的 windowShowWallpaper 透出系统壁纸
    // （WebView 本身仍需单独置透明，见 injectSafeAreaToWebView）
    window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))

    // 延迟执行，等待 Tauri WebView 创建完成
    window.decorView.post { injectSafeAreaToWebView() }
  }

  /** 递归查找 WebView，并注入安全区 CSS 变量 */
  private fun injectSafeAreaToWebView() {
    val webView = findWebView(window.decorView as ViewGroup) ?: return

    // 关键：Android WebView 默认背景是不透明的白色，必须置透明才能透出系统壁纸。
    // 前端 html/body 已设为 transparent，这里兜底去掉 WebView 的白底。
    // 硬件加速下 setBackgroundColor 即生效，无需 setLayerType(OVERLAY)（软件渲染，性能差）。
    webView.setBackgroundColor(Color.TRANSPARENT)

    ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
      val bars = insets.getInsets(systemBars())
      val density = resources.displayMetrics.density

      val js = buildString {
        append("(function(){var e=document.documentElement;")
        append("e.style.setProperty('--safe-area-inset-top','${bars.top / density}px');")
        append("e.style.setProperty('--safe-area-inset-bottom','${bars.bottom / density}px');")
        append("e.style.setProperty('--safe-area-inset-left','${bars.left / density}px');")
        append("e.style.setProperty('--safe-area-inset-right','${bars.right / density}px');")
        append("})()")
      }

      webView.evaluateJavascript(js, null)
      insets
    }

    // 主动触发一次，确保初始值注入
    webView.requestApplyInsets()
  }

  private fun findWebView(parent: ViewGroup): WebView? {
    for (i in 0 until parent.childCount) {
      val child = parent.getChildAt(i)
      when {
        child is WebView -> return child
        child is ViewGroup -> {
          findWebView(child)?.let { return it }
        }
      }
    }
    return null
  }
}
