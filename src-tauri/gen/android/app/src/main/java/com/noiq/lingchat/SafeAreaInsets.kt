package com.noiq.lingchat

import android.util.Log
import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.ScriptHandler
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import java.util.WeakHashMap

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
 * ## ★ 注入时机：必须覆盖「页面还没加载」这一段（2026-10-03 二修）
 *
 * 早先只在 insets 分发时 `evaluateJavascript` 一次。但 WebView 的 insets 是
 * **WebView 被加进视图树那一刻**就分发的，而页面（`loadUrl`）往往几百毫秒后
 * 才建出 document —— 那一次注入落在 `about:blank` 的 document 上，
 * 真页面一加载就没了。
 *
 * 真机表现（用户原话）：「设置页的安全区还是需要**旋转一下屏幕**」——
 * 旋转会触发 `requestApplyInsets`，那时页面已经在了，于是「转一下就好了」。
 *
 * 现在三条路一起兜：
 *
 * 1. **insets 分发时**立即注入（处理旋转、分屏等运行时变化）；
 * 2. **`addDocumentStartJavaScript`** —— 注册成「每个新 document 开始时都跑」
 *    的脚本，值内联在脚本里，insets 变了就重新注册一次。这是**主路径**，
 *    因为它在构造上就保证了「页面无论何时加载都带着当前值」；
 * 3. `attach` 里两次 `postDelayed` 补注入，给极老 WebView（不支持 document-start
 *    脚本）兜底。
 *
 * 脚本自身也做了防御：`documentElement` 还没建出来时挂到
 * `DOMContentLoaded` / `load` 上再跑一次。
 *
 * ## ★ 挖孔只认「主导边」（2026-10-03 二修）
 *
 * 早期是四边都 `max(systemBars, displayCutout)`。但 `DisplayCutout` 会把
 * **同一块挖孔在相邻边上的投影**也报出来：
 *
 * - 竖屏、顶部打孔：`top` 是挖孔高度，`left`/`right` 可能是挖孔在角上的投影；
 * - 竖屏、右上角打孔：`right` 非 0，但它只覆盖屏幕右上角那一小块；
 * - 横屏、挖孔在某侧：那一侧才是挖孔真正占据的整条边。
 *
 * 照单全收的后果就是给**整条边**加出一条内缩。真机表现（用户原话）：
 * 「竖屏下右边会莫名其妙多一点空白」。
 *
 * 所以只取四边里**数值最大**的那个方向当作「挖孔所在边」，其余方向置零。
 * `systemBars` 不受影响（它本来就是整边语义）。
 *
 * ## 为什么必须 `max(systemBars, displayCutout)`
 *
 * 695cd804 给 `Theme.ling_chat` 加了 `windowLayoutInDisplayCutoutMode=always`
 * （为了修悬浮窗横屏被信箱化 → 左侧双份空白 / 右侧溢出）。改成 `always` 之后
 * 窗口**铺满整块屏幕、含挖孔**，而 `Type.systemBars()` 的定义是
 * 「状态栏 + 导航栏 + 标题栏」——**根本不含挖孔**。于是横屏时挖孔跑到左右两侧，
 * 而 `systemBars().left/right` 恒为 0 → 内容直接钻到摄像头下面。
 *
 * 悬浮窗那边（`FloatingPetPlugin.safeInsetsPx()`）一直是
 * `maxOf(systemBars, displayCutout)` 的写法，所以悬浮窗没事 —— 这里是把它对齐过来。
 *
 * ## 为什么挂在 WebView 上而不是 Activity 的 decorView 上
 *
 * 变量最终要在页面里生效，`evaluateJavascript` 需要有 WebView 实例；
 * 挂在 WebView 上还能顺带拿到它自己的 `requestApplyInsets()`。
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
 *
 * ⚠️ 所有方法都必须在 **UI 线程**调用（WebView 的 API 要求），
 * 因此下面几个 `WeakHashMap` 不需要加锁。
 */
internal object SafeAreaInsets {
    private const val TAG = "LingChatSafeArea"

    /** 一次算出的四个 CSS 值（px → CSS px，保留一位小数）。 */
    private data class Values(val top: Float, val bottom: Float, val left: Float, val right: Float) {
        /**
         * 幂等注入脚本。
         *
         * 值直接内联成字面量（而不是去问原生），这样同一段脚本既能
         * `evaluateJavascript` 打进当前文档，也能作为 document-start 脚本
         * 在**每个新文档**里自动执行 —— 后者是修「页面加载后才对」的关键。
         */
        val script: String =
            "(function(){function s(){var e=document.documentElement;if(!e)return false;" +
                "e.style.setProperty('--safe-area-inset-top','${top}px');" +
                "e.style.setProperty('--safe-area-inset-bottom','${bottom}px');" +
                "e.style.setProperty('--safe-area-inset-left','${left}px');" +
                "e.style.setProperty('--safe-area-inset-right','${right}px');" +
                "return true;}" +
                // document-start 阶段 documentElement 可能还没建出来
                "if(!s()){document.addEventListener('DOMContentLoaded',s,{once:true});" +
                "window.addEventListener('load',s,{once:true});}})();"
    }

    /** 每个 WebView 最近一次算出的值 —— 页面加载完之后要按它再注入一次。 */
    private val lastValues = WeakHashMap<WebView, Values>()

    /** document-start 脚本句柄；重新注册前必须先 `remove()` 掉旧的。 */
    private val handlers = WeakHashMap<WebView, ScriptHandler>()

    /** 给 [webView] 装上 insets 监听，并主动要一次初始值。可重复调用（监听器后装覆盖先装）。 */
    fun attach(webView: WebView) {
        ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
            apply(webView, insets)
            insets
        }
        // 主动触发一次，确保初始值注入（attach 到窗口后还会再来一次）
        webView.requestApplyInsets()

        // 兜底路径 ③：页面通常在几百毫秒内加载完。若此时 WebView 还不支持
        // document-start 脚本，这两次补注入就是唯一的机会。
        // 可重复调用时最多多跑几次同样的脚本，幂等无害。
        webView.postDelayed({ reapply(webView) }, 600)
        webView.postDelayed({ reapply(webView) }, 1800)
    }

    /**
     * 用最近一次算出的值重新注入**当前**文档。
     *
     * 供 `onConfigurationChanged` / 布局变化等「文档没换但需要刷新」的场景调用。
     */
    fun reapply(webView: WebView) {
        val values = lastValues[webView] ?: return
        try {
            webView.evaluateJavascript(values.script, null)
        } catch (t: Throwable) {
            Log.w(TAG, "重新注入安全区失败（可忽略）", t)
        }
    }

    private fun apply(webView: WebView, insets: WindowInsetsCompat) {
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
        // ★ 挖孔必须单独取：它不在 systemBars 的定义里，见头注释。
        val cut = insets.getInsets(WindowInsetsCompat.Type.displayCutout())

        // ★ 只认主导边 —— 见头注释「挖孔只认主导边」。
        val cutMax = maxOf(cut.left, cut.top, cut.right, cut.bottom)
        fun cutOn(v: Int) = if (cutMax > 0 && v == cutMax) v else 0

        val density = webView.resources.displayMetrics.density
        val values = Values(
            top = cssPx(maxOf(bars.top, cutOn(cut.top)), density),
            bottom = cssPx(maxOf(bars.bottom, cutOn(cut.bottom)), density),
            left = cssPx(maxOf(bars.left, cutOn(cut.left)), density),
            right = cssPx(maxOf(bars.right, cutOn(cut.right)), density),
        )

        // 真机排查靠这行：`adb logcat -s LingChatSafeArea`
        Log.i(
            TAG,
            "insets bars=${bars.left}/${bars.top}/${bars.right}/${bars.bottom} " +
                "cutout=${cut.left}/${cut.top}/${cut.right}/${cut.bottom} (主导边=$cutMax) " +
                "→ css T=${values.top} B=${values.bottom} L=${values.left} R=${values.right}",
        )

        lastValues[webView] = values

        // 路径 ①：当前文档（旋转、分屏等运行时变化）
        try {
            webView.evaluateJavascript(values.script, null)
        } catch (t: Throwable) {
            Log.w(TAG, "注入安全区失败（可忽略，页面加载后还有一次）", t)
        }

        // 路径 ②：之后**每一个**新文档
        registerDocumentStart(webView, values)
    }

    /**
     * 把当前值注册成「每个新 document 开始时自动执行」的脚本。
     *
     * 这是修「页面加载完才正确 / 要旋转一下屏幕」的**主路径**：
     * 页面无论何时加载，脚本都已经带着当前值等在那里了。
     */
    private fun registerDocumentStart(webView: WebView, values: Values) {
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            Log.w(TAG, "当前 WebView 不支持 DOCUMENT_START_SCRIPT，只能靠首帧 + 延迟补注入")
            return
        }
        try {
            // 同名脚本后注册覆盖先注册：先摘掉旧的，避免越积越多
            handlers.remove(webView)?.remove()
            handlers[webView] = WebViewCompat.addDocumentStartJavaScript(
                webView,
                values.script,
                setOf("*"),
            )
        } catch (t: Throwable) {
            Log.w(TAG, "注册 document-start 脚本失败（可忽略，仍会首帧注入）", t)
        }
    }

    /** px → CSS px。WebView 的 CSS 像素 = 逻辑像素，所以要除 density。 */
    private fun cssPx(px: Int, density: Float): Float {
        if (px <= 0) return 0f
        // 保留一位小数：除 density 会得到 23.999998 这种长尾，日志与样式都不好看
        return Math.round(px / density * 10f) / 10f
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
