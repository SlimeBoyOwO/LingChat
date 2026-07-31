package com.noiq.lingchat

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

    // 延迟执行，等待 Tauri WebView 创建完成
    window.decorView.post { injectSafeAreaToWebView() }
  }

  /** 递归查找 WebView，并注入安全区 CSS 变量 */
  private fun injectSafeAreaToWebView() {
    val webView = findWebView(window.decorView as ViewGroup) ?: return

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
