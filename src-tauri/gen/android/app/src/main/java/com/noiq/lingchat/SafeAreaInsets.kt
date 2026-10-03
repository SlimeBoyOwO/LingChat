package com.noiq.lingchat

import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * 把系统栏（状态栏 / 手势条）**以及挖孔**的安全区尺寸，以 CSS 变量形式注入 WebView。
 *
 * ## 为什么不能靠 `env(safe-area-inset-*)`
 *
 * `src/assets/styles/base.css` 里 `--safe-area-inset-*` 的默认值是
 * `env(safe-area-inset-*, 0px)`，但 **Android WebView 的 `env()` 取不到宿主的
 * insets**（它不参与 `WindowInsets` 分发），实测恒为 `0px`。所以必须由原生
 * 侧读一次真实 insets，再写成行内 CSS 变量覆盖掉 `:root` 的默认值。
 *
 * ## ★ 为什么必须 `max(systemBars, displayCutout)`（2026-10-03 回归）
 *
 * 早期只取 `Type.systemBars()`，这在**没有** `windowLayoutInDisplayCutoutMode=always`
 * 时够用：Android 14 及以下默认会把窗口在挖孔那一侧「信箱化」，系统替我们留了白。
 *
 * 但 695cd804 给 `Theme.ling_chat` 加了 `windowLayoutInDisplayCutoutMode=always`
 * （为了修悬浮窗横屏被信箱化 → 左侧双份空白 / 右侧溢出）。改成 `always` 之后
 * 窗口**铺满整块屏幕、含挖孔**，而 `Type.systemBars()` 的定义是
 * 「状态栏 + 导航栏 + 标题栏」——**根本不含挖孔**。于是：
 *
 * · 竖屏：状态栏通常比挖孔高，看不出来；
 * · 横屏：挖孔跑到左右两侧，而 `systemBars().left/right` 恒为 0
 *   → 内容直接钻到摄像头下面。
 *
 * 真机症状就是「LC 本身的安全区没了」。
 *
 * 悬浮窗那边（`FloatingPetPlugin.safeInsetsPx()`）一直是
 * `maxOf(systemBars, displayCutout)` 的写法，所以悬浮窗没事 —— 这里是把它对齐过来。
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
 *
 * 消费方见 `base.css` 的 `.pt-safe / .pb-safe / .pl-safe / .pr-safe`
 * （LC 自己的设置页 `SettingsNav.vue`、桌宠设置页 `SettingsPage.vue` 在用）。
 */
internal object SafeAreaInsets {
    /** 给 [webView] 装上 insets 监听，并主动要一次初始值。可重复调用（监听器后装覆盖先装）。 */
    fun attach(webView: WebView) {
        ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
            apply(webView, insets)
            insets
        }
        // 主动触发一次，确保初始值注入（attach 到窗口后还会再来一次）
        webView.requestApplyInsets()
    }

    private fun apply(webView: WebView, insets: WindowInsetsCompat) {
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
        // ★ 挖孔必须单独取：它不在 systemBars 的定义里，见头注释。
        val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())

        // px → CSS px（WebView 的 CSS 像素 = 逻辑像素，所以要除 density）
        val density = webView.resources.displayMetrics.density
        val top = maxOf(bars.top, cutout.top)
        val bottom = maxOf(bars.bottom, cutout.bottom)
        val left = maxOf(bars.left, cutout.left)
        val right = maxOf(bars.right, cutout.right)

        val js = buildString {
            append("(function(){var e=document.documentElement;")
            append("e.style.setProperty('--safe-area-inset-top','${top / density}px');")
            append("e.style.setProperty('--safe-area-inset-bottom','${bottom / density}px');")
            append("e.style.setProperty('--safe-area-inset-left','${left / density}px');")
            append("e.style.setProperty('--safe-area-inset-right','${right / density}px');")
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
