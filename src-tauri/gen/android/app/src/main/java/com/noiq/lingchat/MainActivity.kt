package com.noiq.lingchat

import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.os.SystemClock
import android.provider.Settings
import android.util.Log
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import com.noiq.floatingpet.PetVoice
import com.noiq.floatingpet.ScreenCaptureManager
import java.lang.ref.WeakReference

private const val TAG = "LingChatMain"

/**
 * 屏幕截取自检的 intent extra 名。
 *
 * 用法见 [runScreenSelfTest]。
 */
private const val EXTRA_SCREEN_SELFTEST = "lingchat_screen_selftest"

class MainActivity : TauriActivity() {
  /** [onWebViewCreate] 里拿到的 WebView，供转屏后强制重排用。 */
  private var mainWebView: WebView? = null

  /** 已经挂过尺寸监听的 WebView，避免 `prepareWebView` 被两条路径调用时重复挂。 */
  private var watchedWebView: WebView? = null

  /** 上一次已知的 WebView 尺寸。只有真的变了才派发 resize，见 [watchSizeChanges]。 */
  private var lastWebViewW = 0
  private var lastWebViewH = 0

  /** 自检在等授权框回来：`onActivityResult` 里接着跑取帧。 */
  private var pendingScreenSelfTest = false

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

    // 屏幕截取自检入口：见 maybeRunScreenSelfTest。
    maybeRunScreenSelfTest(intent)
  }

  /**
   * `launchMode` 是默认的 `standard`，所以「再次启动」有两种可能：复用了现有实例
   * （走到这里）或者真的新建了一个（走 `onCreate`）。两条路都接上，自检才稳。
   */
  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    maybeRunScreenSelfTest(intent)
  }

  override fun onDestroy() {
    // 只清掉指向自己的那份引用；重建后的新实例会在自己的 onCreate 里重新登记
    if (instanceRef?.get() === this) instanceRef = null
    super.onDestroy()
  }

  /**
   * 屏幕截取（MediaProjection）的授权结果回调。
   *
   * ## 为什么落在本类
   *
   * `MediaProjection` 的授权框只能由**某个 Activity** 用
   * `startActivityForResult` 拉起（Service / Application 都不行），结果也只会
   * 回到发起它的那个 Activity。插件侧的 `FloatingPetPlugin.requestScreenCapture`
   * 是在 `activity`（就是本类实例）上发的 intent，所以回调必然落在这里。
   *
   * ## 为什么可以安全覆写
   *
   * `WryActivity`（`generated/` 下，自动生成）和 `TauriActivity` 都**没有**
   * 覆写本方法 —— 已 grep 确认。`AppCompatActivity` 的实现只做「分发给
   * Fragment」，本项目没有用 Fragment，所以不会抢到任何东西。
   *
   * ## 授权结果不回给前端
   *
   * 弹框是异步的，`requestScreenCapture` 命令在拉起弹框那一刻就已经 resolve
   * 了（`reason=prompted`）。用户点「允许/拒绝」的结果只更新单例状态，
   * 前端要靠下一次 `captureScreen` 的返回值反推（`ok=true` vs
   * `reason=permission_required`）。
   */
  override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
    if (requestCode == ScreenCaptureManager.REQUEST_CODE) {
      val granted = ScreenCaptureManager.onPermissionResult(this, resultCode, data)
      Log.i(TAG, "屏幕截取授权回调：granted=$granted resultCode=$resultCode")

      if (!granted) {
        // 用户在授权框上点了「取消 / 返回」。必须给个反馈 ——
        // 否则弹框一关，用户完全不知道这件事其实没成。
        notifyPermissionDenied("screen")
        pendingScreenSelfTest = false
        return
      }

      if (pendingScreenSelfTest) {
        pendingScreenSelfTest = false
        runScreenSelfTestCapture()
      }
      return
    }
    super.onActivityResult(requestCode, resultCode, data)
  }

  /**
   * 把「权限被拒」告诉页面，由页面弹**应用内 toast**。
   *
   * ## 为什么不用系统 Toast（2026-10-17 用户要求改掉）
   *
   * 系统 Toast 是灰底黑字的系统气泡，跟 App 的视觉完全脱节，也不受主题控制。
   * 而这两次拒绝其实都发生在**页面可用**的时刻：
   *
   * - 悬浮窗权限：从系统设置回来，Activity 已 resume、WebView 已恢复；
   * - 屏幕截取：授权框关掉，Activity 回到前台。
   *
   * 所以推一个 `CustomEvent` 给页面，让它用自己的 `uiStore.showWarning`
   * 弹 toast —— 样式、语气、主题都与 App 一致。
   *
   * 页面没接住（比如 WebView 恰好在重载）就静默丢掉：这只是提示，
   * 丢了不影响任何功能。
   *
   * @param which `overlay`（悬浮窗权限）/ `screen`（屏幕截取）
   */
  private fun notifyPermissionDenied(which: String) {
    val webView = mainWebView
    if (webView == null) {
      Log.w(TAG, "页面还没就绪，丢弃权限拒绝提示（which=$which）")
      return
    }
    val js =
      "window.dispatchEvent(new CustomEvent('pet-permission-denied'," +
        "{detail:{which:'$which'}}))"
    try {
      webView.evaluateJavascript(js, null)
      Log.i(TAG, "已通知页面：权限被拒（which=$which）")
    } catch (t: Throwable) {
      Log.w(TAG, "派发权限拒绝提示失败（可忽略）", t)
    }
  }

  /**
   * 悬浮窗权限的「拒绝」检测。
   *
   * `ACTION_MANAGE_OVERLAY_PERMISSION` 打开的是系统设置页，**不会回传结果**
   * （它不是 `startActivityForResult` 语义）。所以只能这样判：插件在发起跳转时
   * 置位 [PetVoice.awaitingOverlayPermission]，回到前台时再看权限到底有没有拿到。
   *
   * 没拿到 → 让页面弹一句撒娇台词（应用内 toast，不是系统 Toast）。
   * 拿到了就静默放过（正常流程，不需要提示）。
   */
  override fun onResume() {
    super.onResume()
    if (PetVoice.awaitingOverlayPermission) {
      PetVoice.awaitingOverlayPermission = false
      if (!Settings.canDrawOverlays(this)) {
        Log.i(TAG, "用户没有授予悬浮窗权限，通知页面提示一句")
        notifyPermissionDenied("overlay")
      }
    }
  }

  // ─── 屏幕截取自检（运维/调试入口）────────────────────────────

  /**
   * 用 adb 触发一次「授权 + 取帧」全链路自检，结果只写 logcat。
   *
   * ```bash
   * adb shell am start -n com.noiq.lingchat/.MainActivity \
   *     -a android.intent.action.MAIN -c android.intent.category.LAUNCHER \
   *     --ez lingchat_screen_selftest true
   * adb logcat -s LingChatMain ScreenCapture
   * ```
   *
   * ## 为什么需要它
   *
   * `MediaProjection` 的授权是**进程级**的：每次冷启动都要重新点一次「允许」。
   * 而正式链路（主动策略）是后台定时触发的 —— 想验证一次，得先开主动对话、
   * 再把屏幕权重调大于 0、然后等策略随机选中 SCREEN，可能要等好几分钟。
   * 这个入口把「授权 + 取帧」压成一条命令，几秒内就能看到结果。
   *
   * ## 为什么不落盘
   *
   * 自检只把**字节数 / 耗时 / JPEG 头**写进日志。
   * 不写文件是刻意的：`MainActivity` 是 `exported="true"`（launcher 需要），
   * 任何 App 都能发这个 intent。只要不落盘，最坏情况也只是「弹一次授权框」，
   * 而授权框本身就要用户点「允许」—— 没有静默取景的可能。
   *
   * （正常链路 `captureScreen` 会把 JPEG 写进 `cacheDir` 给 Rust 读，
   * 那条路径只有已授权的进程内调用能走，与这里无关。）
   */
  private fun maybeRunScreenSelfTest(intent: Intent?) {
    if (intent?.getBooleanExtra(EXTRA_SCREEN_SELFTEST, false) != true) return
    // 消费掉，避免 Activity 重建（转屏等）时重复触发
    intent.removeExtra(EXTRA_SCREEN_SELFTEST)

    Log.i(TAG, "屏幕截取自检：开始（isReady=${ScreenCaptureManager.isReady()}）")

    if (ScreenCaptureManager.isReady()) {
      runScreenSelfTestCapture()
      return
    }

    val request = ScreenCaptureManager.requestIntent(this)
    if (request == null) {
      Log.w(TAG, "屏幕截取自检：系统不支持 MediaProjection")
      return
    }
    pendingScreenSelfTest = true
    startActivityForResult(request, ScreenCaptureManager.REQUEST_CODE)
  }

  /** 在后台线程取一帧，把可判断的信息写进日志（阻塞上限 3s，不能占主线程）。 */
  private fun runScreenSelfTestCapture() {
    Thread {
      // token 是在前台服务的 onStartCommand 里换的（系统要求），
      // 所以 onActivityResult 返回时可能还没就绪，先等一小会儿。
      val readyDeadline = SystemClock.uptimeMillis() + 3_000
      while (!ScreenCaptureManager.isReady() && SystemClock.uptimeMillis() < readyDeadline) {
        Thread.sleep(50)
      }
      if (!ScreenCaptureManager.isReady()) {
        Log.w(TAG, "屏幕截取自检：等 3s 仍未就绪，放弃")
        return@Thread
      }

      val started = SystemClock.uptimeMillis()
      val bytes = ScreenCaptureManager.captureJpeg(applicationContext)
      val cost = SystemClock.uptimeMillis() - started

      if (bytes == null) {
        Log.w(TAG, "屏幕截取自检：取帧失败（${cost}ms）")
        return@Thread
      }

      // JPEG 一定以 FF D8 FF 开头；不是的话说明拿到的不是一张 JPEG。
      val magic = bytes.take(3).joinToString("") { "%02X".format(it.toInt() and 0xFF) }
      // 全黑图压到 JPEG q85 通常只有几 KB；一张真实界面在 1024 长边下是 80~300 KB。
      // 所以字节数本身就是「是不是真截到画面」的强信号。
      val verdict = if (bytes.size > 30_000) "看起来正常" else "偏小，可能画面全黑"
      Log.i(
        TAG,
        "屏幕截取自检：成功 ${bytes.size} 字节，耗时 ${cost}ms，JPEG 头=$magic，$verdict",
      )
    }.start()
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
