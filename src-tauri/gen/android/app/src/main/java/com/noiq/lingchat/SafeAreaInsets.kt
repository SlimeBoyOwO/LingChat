package com.noiq.lingchat

import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * 把系统栏（状态栏 / 手势条）的安全区尺寸，以 CSS 变量形式注入 WebView。
 *
 * ## 为什么不能靠 `env(safe-area-inset-*)`
 *
 * `src/assets/styles/base.css` 里 `--safe-area-inset-*` 的默认值是
 * `env(safe-area-inset-*, 0px)`，但 **Android WebView 的 `env()` 取不到宿主的
 * insets**（它不参与 `WindowInsets` 分发），实测恒为 `0px`。所以必须由原生
 * 侧读一次真实 insets，再写成行内 CSS 变量覆盖掉 `:root` 的默认值。
 *
 * ## 为什么挂在 WebView 上而不是 Activity 的 decorView 上
 *
 * 变量最终要在页面里生效，`evaluateJavascript` 需要有 WebView 实例；
 * 挂在 WebView 上还能顺带拿到它自己的 `requestApplyInsets()`。
 * 监听器在 WebView 被 attach 到窗口时一定会被调用一次，所以即使
 * [attach] 早于 attach 到视图树执行也没问题（`requestApplyInsets` 只是
 * 「尽快来一次」的提示，不是唯一触发点）。
 *
 * ## 谁在用
 *
 * - [MainActivity]（桌宠悬浮窗 / 聊天主界面，透壁纸的全屏窗口）
 * - [SettingsActivity]（桌宠「设置」独立窗口）
 *
 * 两个 Activity 都必须调用：`targetSdk = 36`，Android 15+ 对 targetSdk ≥ 35 的
 * 应用**强制 edge-to-edge**，任何 Activity 都会铺到系统栏下面，
 * 不注入就一定会被状态栏 / 手势条压住。
 */
internal object SafeAreaInsets {
    /** 给 [webView] 装上 insets 监听，并主动要一次初始值。可重复调用（监听器后装覆盖先装）。 */
    fun attach(webView: WebView) {
        ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
            apply(webView, insets.getInsets(WindowInsetsCompat.Type.systemBars()))
            insets
        }
        // 主动触发一次，确保初始值注入（attach 到窗口后还会再来一次）
        webView.requestApplyInsets()
    }

    private fun apply(webView: WebView, bars: Insets) {
        // px → CSS px（WebView 的 CSS 像素 = 逻辑像素，所以要除 density）
        val density = webView.resources.displayMetrics.density

        val js = buildString {
            append("(function(){var e=document.documentElement;")
            append("e.style.setProperty('--safe-area-inset-top','${bars.top / density}px');")
            append("e.style.setProperty('--safe-area-inset-bottom','${bars.bottom / density}px');")
            append("e.style.setProperty('--safe-area-inset-left','${bars.left / density}px');")
            append("e.style.setProperty('--safe-area-inset-right','${bars.right / density}px');")
            append("})()")
        }

        webView.evaluateJavascript(js, null)
    }

    /** 递归查找 WebView。给那些拿不到 `onWebViewCreate` 时机的场景兜底用。 */
    fun findWebView(parent: ViewGroup): WebView? {
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
