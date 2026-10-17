package com.noiq.floatingpet

import android.accessibilityservice.AccessibilityService
import android.graphics.Bitmap
import android.os.Build
import android.util.Log
import android.view.Display
import android.view.accessibility.AccessibilityEvent
import androidx.annotation.RequiresApi
import java.io.ByteArrayOutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

private const val TAG = "FloatingPet"

/**
 * 「屏幕视奸」（主动窥屏）在 Android 上的**取景通道**。
 *
 * ## 它解决的是什么
 *
 * 项目里本来就有一套「视觉感受 / 主动窥屏」（桌面端 README 的
 * 「👀 视觉感受与主动窥屏」）：主动对话系统每 30s 轮询一次，按权重随机挑一种
 * 策略，挑中 `SCREEN` 时**截一帧屏幕**交给对话模型（原生多模态当轮直发）或
 * 视觉模型（旁白转述）。
 *
 * 但那条链路在 Android 上**整条不触发** —— `crates/ling-chat-main` 的
 * `capture_screen_as_jpeg()` 是 `#[cfg(not(target_os = "windows"))] { None }`，
 * `strategy_dispatcher` 拿到 `None` 就 `?` 早退。本服务补的就是「截一帧」这一步。
 *
 * ## 为什么是无障碍服务，而不是 MediaProjection
 *
 * 两条路都能截屏，差别在**授权能不能记住**：
 *
 * | 维度 | 无障碍 `takeScreenshot` | `MediaProjection` |
 * | --- | --- | --- |
 * | 授权方式 | 系统设置里开一次服务 | 每次会话弹「开始投屏」 |
 * | App 冷启动后 | 仍然有效 | **必须重新授权** |
 * | Android 14+ | 无额外要求 | 必须先起 `mediaProjection` 前台服务 |
 * | 附加能力 | 能顺带拿到前台包名 | 只有画面 |
 * | 上架 Google Play | 审核极严（易被拒） | 无政策风险 |
 *
 * 对一个「后台默默窥屏」的功能，「每次冷启动都被弹一次投屏授权」是不可接受的，
 * 所以选前者。`MediaProjection` 留作将来的第二通道 —— 真要上架时再补，两者
 * 可以在 `capture_screen_as_jpeg()` 里按可用性择优共存。
 *
 * ## ⚠️ 两个硬约束
 *
 * 1. **`takeScreenshot` 要 API 30（Android 11）**。更低版本上本服务依然能连上，
 *    只是截不了图 —— [captureJpeg] 会返回 `null`，调用方应当降级（比如只用
 *    [foregroundPackage] 报「用户在用什么 App」）。
 * 2. **必须由用户在系统设置里手动开启**（设置 → 无障碍 → 已安装的服务）。
 *    这是 Android 对无障碍服务的强制要求：App 既不能自行开启，也没有授权弹窗。
 *    未开启时 [isReady] 为 `false`，[captureJpeg] 直接返回 `null`。
 *
 * ## 隐私边界
 *
 * `takeScreenshot` 只截**画面**，不读控件树（配置里 `canRetrieveWindowContent`
 * 保持 `false`）。截到的内容只用于当轮对话，落盘位置是 App 私有的 `cacheDir`
 * （见插件命令 `captureScreen`），不进用户可见目录、不做长期保存。
 */
class LingChatAccessibilityService : AccessibilityService() {

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        Log.i(TAG, "无障碍服务已连接：屏幕感知可用")
    }

    /**
     * 只做一件事：记住**当前前台包名**。
     *
     * 它不参与截图（截图走 [captureJpeg] 的同步入口），只是给窥屏结果加一层
     * 语境 —— 「用户在微信里」比「屏幕上有一片白色」信息量大得多。
     * 拿不到就留空，不阻塞任何流程。
     */
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event?.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED) {
            event.packageName?.toString()?.takeIf { it.isNotEmpty() }?.let {
                foregroundPackage = it
            }
        }
    }

    override fun onInterrupt() = Unit

    override fun onUnbind(intent: android.content.Intent?): Boolean {
        instance = null
        Log.i(TAG, "无障碍服务已断开：屏幕感知不可用")
        return super.onUnbind(intent)
    }

    companion object {
        /** 截图等待上限（秒）。超时按失败处理，绝不让调用方无限等。 */
        private const val CAPTURE_TIMEOUT_SEC = 5L

        /** 输出 JPEG 的最长边。与桌面端 GDI 那条路的 1024×768 同一量级。 */
        private const val MAX_EDGE = 1024

        /** JPEG 质量。85 是体积/清晰度的常用折中。 */
        private const val JPEG_QUALITY = 85

        @Volatile
        private var instance: LingChatAccessibilityService? = null

        /** 最近一次观察到的前台包名；拿不到时为 `null`。 */
        @Volatile
        var foregroundPackage: String? = null
            private set

        /** 无障碍服务是否已连接（= 用户已在系统设置里开启）。 */
        fun isReady(): Boolean = instance != null

        /**
         * 截一帧并编码成 JPEG。
         *
         * @return JPEG 字节；服务未开启 / API < 30 / 截图失败 / 超时都返回 `null`。
         *
         * ⚠️ **这是阻塞调用**，等待上限 [CAPTURE_TIMEOUT_SEC] 秒。调用方必须在
         * 非主线程上用它（Tauri 插件的命令线程可以；主线程不行）。
         */
        fun captureJpeg(): ByteArray? {
            val service = instance
            if (service == null) {
                Log.w(TAG, "无障碍服务未开启，无法截图")
                return null
            }
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
                Log.w(TAG, "API ${Build.VERSION.SDK_INT} < 30，takeScreenshot 不可用")
                return null
            }
            return service.captureJpegInternal()
        }
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun captureJpegInternal(): ByteArray? {
        val latch = CountDownLatch(1)
        val holder = AtomicReference<Bitmap?>(null)
        val executor = Executors.newSingleThreadExecutor()
        try {
            takeScreenshot(
                Display.DEFAULT_DISPLAY,
                executor,
                object : TakeScreenshotCallback {
                    override fun onSuccess(screenshot: ScreenshotResult) {
                        try {
                            val buffer = screenshot.hardwareBuffer
                            // `wrapHardwareBuffer` 拿到的是 GPU 侧位图，**必须 copy 成
                            // ARGB_8888** 才能安全读像素并压缩 —— 直接压它会在
                            // buffer close 之后拿到一张全黑图。
                            val wrapped = Bitmap.wrapHardwareBuffer(buffer, screenshot.colorSpace)
                            holder.set(wrapped?.copy(Bitmap.Config.ARGB_8888, false))
                            buffer.close()
                        } catch (t: Throwable) {
                            Log.w(TAG, "截图结果转换失败", t)
                        } finally {
                            latch.countDown()
                        }
                    }

                    override fun onFailure(errorCode: Int) {
                        Log.w(TAG, "takeScreenshot 失败，errorCode=$errorCode")
                        latch.countDown()
                    }
                },
            )
            if (!latch.await(CAPTURE_TIMEOUT_SEC, TimeUnit.SECONDS)) {
                Log.w(TAG, "takeScreenshot 超时（${CAPTURE_TIMEOUT_SEC}s）")
                return null
            }
        } catch (t: Throwable) {
            Log.w(TAG, "发起截图失败", t)
            return null
        } finally {
            executor.shutdown()
        }
        val bitmap = holder.get() ?: return null
        return compressToJpeg(bitmap)
    }

    /** 等比缩到最长边 [MAX_EDGE] 再压 JPEG，避免整屏原图把请求体撑爆。 */
    private fun compressToJpeg(source: Bitmap): ByteArray? {
        val scaled = scaleToMaxEdge(source, MAX_EDGE)
        return try {
            ByteArrayOutputStream().use { out ->
                scaled.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, out)
                out.toByteArray()
            }
        } catch (t: Throwable) {
            Log.w(TAG, "JPEG 编码失败", t)
            null
        } finally {
            if (scaled !== source) scaled.recycle()
            source.recycle()
        }
    }

    private fun scaleToMaxEdge(source: Bitmap, maxEdge: Int): Bitmap {
        val longEdge = maxOf(source.width, source.height)
        if (longEdge <= maxEdge) return source
        val ratio = maxEdge.toDouble() / longEdge
        val w = (source.width * ratio).toInt().coerceAtLeast(1)
        val h = (source.height * ratio).toInt().coerceAtLeast(1)
        return Bitmap.createScaledBitmap(source, w, h, true)
    }
}
