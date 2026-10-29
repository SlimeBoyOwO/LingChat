package com.noiq.floatingpet

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.Image
import android.media.ImageReader
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Handler
import android.os.HandlerThread
import android.os.SystemClock
import android.util.DisplayMetrics
import android.util.Log
import android.view.Display
import java.io.ByteArrayOutputStream

/**
 * 「屏幕视奸」（主动窥屏）的取景通道 —— **MediaProjection** 实现。
 *
 * ## 为什么是这条路
 *
 * Android 上能拿到「别的 App 的画面」的官方路子只有三条：
 *
 * | 通道 | 最低版本 | 授权形态 | 代价 |
 * | --- | --- | --- | --- |
 * | `AccessibilityService.takeScreenshot` | API 30 | 去系统设置开无障碍，一次永久 | 权限极重，属「大材小用」；部分 ROM / 虚拟化环境会挡 |
 * | `MediaProjection`（本类） | API 21 | **应用内弹「允许开始截取屏幕内容」** | 每次进程重启要重授 |
 * | `PixelCopy` | API 24 | 无需授权 | **只能截自己进程**，拿不到别的 App |
 *
 * 本项目选第二条：授权框由应用自己拉起，不用把用户赶去系统设置，
 * 语义也正好对上「允许应用开始获取屏幕上的内容」这件事本身。
 *
 * ## 生命周期（最容易踩的地方）
 *
 * `MediaProjection` **不能**随时 new，只能由
 * `MediaProjectionManager.getMediaProjection(resultCode, data)` 从一次用户授权
 * 的结果里换出来，而那次授权必须由**某个 Activity** 用
 * `startActivityForResult` 发起 —— Service / Application 都不行。
 *
 * 所以链路是：
 * ```
 * 进入桌宠（用户点了一下桌宠）
 *      → 插件命令 requestScreenCapture
 *      → activity.startActivityForResult(createScreenCaptureIntent())
 *      → 用户点「立即开始」
 *      → MainActivity.onActivityResult
 *      → ScreenCaptureService.start(授权结果)          ← 前台服务（系统硬要求）
 *      → onStartCommand: startForeground(TYPE_MEDIA_PROJECTION)
 *      → ScreenCaptureManager.adoptProjection(...)     ← 此时才 getMediaProjection
 *      → 之后 captureJpeg() 随时可用
 * ```
 *
 * ⚠️ 中间那个前台服务**不是可选项**：从 Android 10（API 29）起，
 * `getMediaProjection()` 要求调用那一刻已有 `mediaProjection` 类型的前台服务
 * 在跑，否则抛 `SecurityException`。详见 [ScreenCaptureService] 的说明。
 *
 * ## 为什么每次截图都临时建 VirtualDisplay
 *
 * 也可以长期持有一个 `VirtualDisplay`，但那样屏幕内容会**持续**被合成到一个
 * Surface 上 —— 对「桌宠偶尔瞄一眼」这种低频场景纯属浪费（耗电、占 GPU）。
 * 这里改成「用时建、截完立刻释放」，`MediaProjection` 本身留着复用，
 * 代价只有建/拆 VirtualDisplay 的几十毫秒。
 *
 * ## 拿不到的东西
 *
 * MediaProjection 只给**画面**，不给前台包名。所以
 * `ScreenCapture.foregroundPackage` 走这条通道时永远是空串 ——
 * 调用方不要依赖它做判断（它只用于日志/提示）。
 */
object ScreenCaptureManager {
    private const val TAG = "ScreenCapture"

    /** `startActivityForResult` 的请求码，`MainActivity.onActivityResult` 用它分流。 */
    const val REQUEST_CODE = 0x4C43 // "LC"

    /** 等第一帧的最长时间。VirtualDisplay 建好后通常 1~2 帧内就有画面。 */
    private const val FRAME_WAIT_MS = 3000L

    /** 轮询帧的间隔。`acquireLatestImage` 没新帧时返回 null，不是阻塞调用。 */
    private const val FRAME_POLL_MS = 40L

    /** 长边上限。1024 足够模型看清「在干什么」，再大只是徒增 token。 */
    private const val MAX_EDGE = 1024

    private const val JPEG_QUALITY = 85

    @Volatile
    private var projection: MediaProjection? = null

    /**
     * 回调线程。
     *
     * `registerCallback` 需要 Handler；给 null 的话回调走主线程，而 `onStop`
     * 里要拆东西 —— 不该占主线程。
     */
    @Volatile
    private var callbackThread: HandlerThread? = null

    @Volatile
    private var callbackHandler: Handler? = null

    /** 拿 token 时的上下文，`release()` 要用它停前台服务。 */
    @Volatile
    private var appContext: Context? = null

    /**
     * 是否已经拿到授权（拿到之后才能截）。
     *
     * ## 为什么这里没有「是否已经问过」的标记
     *
     * 2026-10-17 之前有一对 `promptShown` / `shouldPrompt()` / `markPromptShown()`，
     * 用来让原生侧自动弹授权框时别反复骚扰。现在授权时机收口到**前端门禁**
     * （进桌宠前先查 [isReady]，没授权就弹说明 + 拉授权框），「问没问过」这件事
     * 由前端 `MainChat.goToPetMode` 的 `screenCaptureAsked` 持有。
     *
     * 留一个没人写的布尔在这里，只会让后来者误以为「标记为 true 就代表问过了」，
     * 所以直接删掉。
     *
     * ⚠️ 授权是**进程级**的：进程重启后 [isReady] 必然回到 `false`，需要重新授权。
     * 这是 MediaProjection 的系统约束，不是 bug。
     */
    fun isReady(): Boolean = projection != null

    /**
     * 构造「请求截屏授权」的 Intent。
     *
     * 本方法**不启动**任何东西，调用方负责 `startActivityForResult`。
     * 返回 null 表示系统不给这个能力（理论上只有极老的 ROM）。
     */
    fun requestIntent(context: Context): Intent? {
        val manager = context.getSystemService(Context.MEDIA_PROJECTION_SERVICE)
            as? MediaProjectionManager ?: run {
            Log.w(TAG, "拿不到 MediaProjectionManager")
            return null
        }
        return try {
            manager.createScreenCaptureIntent()
        } catch (t: Throwable) {
            Log.w(TAG, "createScreenCaptureIntent 失败", t)
            null
        }
    }

    /**
     * 处理授权结果。由 `MainActivity.onActivityResult` 转发。
     *
     * ## 这里**不**换 token
     *
     * 系统从 Android 10（API 29）起要求：调用 `getMediaProjection()` 的那一刻，
     * 本 App 必须已经有一个 `foregroundServiceType="mediaProjection"` 的前台服务
     * 在跑，否则抛 `SecurityException`。而前台服务的 `startForeground` 是异步的
     * （要等 `onStartCommand`），没法在 `onActivityResult` 里同步完成。
     *
     * 所以这里只做两件事：确认用户点了「允许」，把授权结果交给
     * [ScreenCaptureService]；真正的 `getMediaProjection` 在服务里、
     * `startForeground` 之后执行（见 [adoptProjection]）。
     *
     * 代价：本方法返回 true 之后，[isReady] 可能还要过几十毫秒才变 true。
     *
     * @return 用户是否点了「允许」，且前台服务是否启动成功
     */
    fun onPermissionResult(context: Context, resultCode: Int, data: Intent?): Boolean {
        if (resultCode != Activity.RESULT_OK || data == null) {
            Log.i(TAG, "用户取消了屏幕截取授权（resultCode=$resultCode）")
            return false
        }
        appContext = context.applicationContext
        return ScreenCaptureService.start(context, resultCode, data)
    }

    /**
     * 真正把授权结果换成 `MediaProjection`。**只能由 [ScreenCaptureService]
     * 在 `startForeground` 之后调用**。
     *
     * @return 是否拿到了可用的 projection
     */
    fun adoptProjection(context: Context, resultCode: Int, data: Intent): Boolean {
        val manager = context.getSystemService(Context.MEDIA_PROJECTION_SERVICE)
            as? MediaProjectionManager ?: run {
            Log.w(TAG, "拿不到 MediaProjectionManager")
            return false
        }

        // 先拆旧的：同一个进程里换 token 时，旧的必须 stop，否则系统会
        // 累积多个投屏会话（状态栏图标也会叠）。
        releaseProjectionOnly()

        val mp = try {
            manager.getMediaProjection(resultCode, data)
        } catch (t: Throwable) {
            Log.w(TAG, "getMediaProjection 失败", t)
            return false
        } ?: run {
            Log.w(TAG, "getMediaProjection 返回 null")
            return false
        }

        val thread = HandlerThread("lingchat-screencap").also { it.start() }
        val handler = Handler(thread.looper)

        // ★ 必须注册 callback。两个理由：
        //   1. Android 14+ 起，没注册 callback 就 createVirtualDisplay 会抛
        //      SecurityException（"Must register a callback before starting
        //      capture"）。在 11 上不强制，但留着可以前向兼容。
        //   2. 用户从系统状态栏点「停止投屏」时，我们得知道 token 已失效，
        //      否则后续 captureJpeg 会一直拿 null 却不知道为什么。
        mp.registerCallback(
            object : MediaProjection.Callback() {
                override fun onStop() {
                    Log.i(TAG, "MediaProjection 被停止（用户从系统 UI 停止投屏）")
                    release()
                }
            },
            handler,
        )

        projection = mp
        callbackThread = thread
        callbackHandler = handler
        appContext = context.applicationContext
        Log.i(TAG, "MediaProjection 授权成功")
        return true
    }

    /**
     * 截一帧并压成 JPEG。**阻塞**，最长 [FRAME_WAIT_MS]。
     *
     * 必须在**非主线程**调用（插件命令默认跑在 Tauri 的 worker 线程上，可以）。
     *
     * @return JPEG 字节；未授权 / 等帧超时 / 转换失败都返回 null
     */
    fun captureJpeg(context: Context): ByteArray? {
        val mp = projection ?: return null
        val size = realScreenSize(context) ?: return null
        val width = size.first
        val height = size.second
        if (width <= 0 || height <= 0) {
            Log.w(TAG, "屏幕尺寸异常：${width}x$height")
            return null
        }
        val densityDpi = context.resources.displayMetrics.densityDpi

        var reader: ImageReader? = null
        var virtualDisplay: VirtualDisplay? = null
        var image: Image? = null
        try {
            // maxImages = 2：够「等新帧」用，又不至于积压。
            reader = ImageReader.newInstance(width, height, PixelFormat.RGBA_8888, 2)
            virtualDisplay = mp.createVirtualDisplay(
                "lingchat-capture",
                width,
                height,
                densityDpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                reader.surface,
                null,
                callbackHandler,
            ) ?: run {
                Log.w(TAG, "createVirtualDisplay 返回 null")
                return null
            }

            val deadline = SystemClock.uptimeMillis() + FRAME_WAIT_MS
            while (SystemClock.uptimeMillis() < deadline) {
                image = reader.acquireLatestImage()
                if (image != null) break
                Thread.sleep(FRAME_POLL_MS)
            }
            val frame = image ?: run {
                Log.w(TAG, "等帧超时（${FRAME_WAIT_MS}ms）")
                return null
            }

            val bitmap = imageToBitmap(frame, width, height) ?: return null
            return compressToJpeg(bitmap)
        } catch (t: Throwable) {
            Log.w(TAG, "截图失败", t)
            return null
        } finally {
            // 顺序：先关 Image，再拆 VirtualDisplay，最后关 Reader。
            // 反过来会在某些 ROM 上抛 "Surface has been released"。
            try {
                image?.close()
            } catch (_: Throwable) {
            }
            try {
                virtualDisplay?.release()
            } catch (_: Throwable) {
            }
            try {
                reader?.close()
            } catch (_: Throwable) {
            }
        }
    }

    /**
     * 取**物理屏幕**的尺寸（不是 App 窗口的尺寸）。
     *
     * ## 为什么不用 `context.resources.displayMetrics`
     *
     * 分屏 / 自由窗口（本项目的调试环境里跑着 `com.sunshine.freeform` 容器）
     * 下，`Resources.displayMetrics` 给的是**窗口**大小。拿它去建
     * `VirtualDisplay` 会得到一张比例不对的画面 —— 系统不会拉伸，而是加黑边。
     *
     * `Display.getRealMetrics()` 拿的是含系统栏的整块屏幕，与
     * `MediaProjection` 镜像的源完全一致。
     *
     * （`getRealMetrics` 在 API 31 标记为 deprecated，但替代品
     * `WindowManager.getMaximumWindowMetrics()` 给的是「应用可能占据的最大区域」，
     * 也不等于物理屏幕。这里要的就是物理屏幕，所以继续用它。）
     */
    @Suppress("DEPRECATION")
    private fun realScreenSize(context: Context): Pair<Int, Int>? {
        val dm = context.getSystemService(Context.DISPLAY_SERVICE) as? DisplayManager
        val display = dm?.getDisplay(Display.DEFAULT_DISPLAY)
        if (display == null) {
            Log.w(TAG, "拿不到默认 Display")
            return null
        }
        val metrics = DisplayMetrics()
        display.getRealMetrics(metrics)
        return metrics.widthPixels to metrics.heightPixels
    }

    /**
     * `Image`（RGBA_8888）→ `Bitmap`。
     *
     * ⚠️ 关键点：`rowStride` 可能**大于** `width * pixelStride`（硬件按行对齐）。
     * 直接按 `width` 建图会让每一行错位，画面变成斜的。必须按 padding 后的
     * 宽度建图，再裁回真实宽度。
     */
    private fun imageToBitmap(image: Image, width: Int, height: Int): Bitmap? {
        val plane = image.planes.firstOrNull() ?: run {
            Log.w(TAG, "Image 没有 plane")
            return null
        }
        val pixelStride = plane.pixelStride
        val rowStride = plane.rowStride
        if (pixelStride <= 0 || rowStride <= 0) {
            Log.w(TAG, "非法 stride：pixel=$pixelStride row=$rowStride")
            return null
        }
        val rowPadding = rowStride - pixelStride * width
        val paddedWidth = width + rowPadding / pixelStride

        val padded = Bitmap.createBitmap(paddedWidth, height, Bitmap.Config.ARGB_8888)
        padded.copyPixelsFromBuffer(plane.buffer)
        if (paddedWidth == width) return padded

        val cropped = Bitmap.createBitmap(padded, 0, 0, width, height)
        padded.recycle()
        return cropped
    }

    /** 缩到长边 [MAX_EDGE] 以内再压 JPEG。 */
    private fun compressToJpeg(source: Bitmap): ByteArray? {
        val scaled = scaleToMaxEdge(source, MAX_EDGE)
        return try {
            val out = ByteArrayOutputStream()
            scaled.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, out)
            out.toByteArray()
        } finally {
            if (scaled !== source) scaled.recycle()
            source.recycle()
        }
    }

    private fun scaleToMaxEdge(source: Bitmap, maxEdge: Int): Bitmap {
        val longEdge = maxOf(source.width, source.height)
        if (longEdge <= maxEdge) return source
        val ratio = maxEdge.toFloat() / longEdge
        val w = (source.width * ratio).toInt().coerceAtLeast(1)
        val h = (source.height * ratio).toInt().coerceAtLeast(1)
        return Bitmap.createScaledBitmap(source, w, h, true)
    }

    /**
     * 彻底释放：放掉 token **并**停掉前台服务。
     *
     * 用户从状态栏点「停止投屏」、或主动关闭屏幕感知时走这里。
     */
    fun release() {
        val ctx = appContext
        // 先清字段再 stop：`stop()` 会触发 [MediaProjection.Callback.onStop]，
        // 而那里也会调本方法 —— 不先置空就是无限递归。
        appContext = null
        releaseProjectionOnly()
        if (ctx != null) ScreenCaptureService.stop(ctx)
    }

    /**
     * 只放 token，不碰前台服务。
     *
     * 由 [ScreenCaptureService.onDestroy] 调用 —— 服务已经在销毁了，再回头
     * `stopService` 就是自己调自己。
     */
    fun detachProjection() {
        appContext = null
        releaseProjectionOnly()
    }

    private fun releaseProjectionOnly() {
        val mp = projection
        projection = null
        try {
            mp?.stop()
        } catch (t: Throwable) {
            Log.w(TAG, "停止 MediaProjection 失败", t)
        }
        callbackThread?.quitSafely()
        callbackThread = null
        callbackHandler = null
    }
}
