package com.noiq.floatingpet

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log

private const val TAG = "ScreenCaptureSvc"
private const val CHANNEL_ID = "lingchat_screen_capture"
private const val NOTIFICATION_ID = 0x53434E // "SCN"

/**
 * 屏幕截取的**前台服务外壳** —— 只为满足系统对 `MediaProjection` 的前置要求。
 *
 * ## 为什么非要有一个前台服务
 *
 * 这是本项目最容易误判的一条硬约束。直觉上「Android 14 才开始要前台服务」，
 * 实际上从 **Android 10（API 29）** 起就要求了。真机（Pixel 4 / Android 11）
 * 日志：
 *
 * ```
 * java.lang.SecurityException: Media projections require a foreground service
 *   of type ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
 *     at android.media.projection.MediaProjectionManager.getMediaProjection(...)
 * ```
 *
 * 也就是说：用户明明点了「立即开始」，`getMediaProjection()` 照样抛异常 ——
 * 除非调用它的**那一刻**，本 App 已经有一个 `foregroundServiceType="mediaProjection"`
 * 的前台服务在跑。顺序不能反：
 *
 * ```text
 * 用户点「立即开始」
 *   → MainActivity.onActivityResult
 *   → startForegroundService(本服务, 带 resultCode + data)
 *   → onStartCommand: startForeground(..., TYPE_MEDIA_PROJECTION)   ← 必须在前
 *   → ScreenCaptureManager.adoptProjection(resultCode, data)        ← getMediaProjection
 * ```
 *
 * ## 为什么不复用 `PetForegroundService`
 *
 * 它启动时声明的是 `specialUse`。系统检查的是「**当前在跑的**前台服务是否带
 * mediaProjection 类型」，不是「manifest 里有没有声明」。要复用就得在截屏时
 * 把它按新类型重启一遍，反而容易把桌宠保活搞出空档。两个服务各管一件事更稳。
 *
 * ## 生命周期
 *
 * `MediaProjection` 的授权是**进程级**的：拿到一次之后，后续 `captureJpeg`
 * 可以反复用，不需要重新弹框。所以本服务跟着授权一起活到进程结束（或用户从
 * 状态栏点「停止投屏」），**不随单次截图起停** —— 否则每截一张就要弹一次框。
 */
class ScreenCaptureService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // 1. 先把自己变成「带 mediaProjection 类型的前台服务」。
        //    顺序反了就是上面那个 SecurityException。
        if (!promoteToForeground()) {
            Log.w(TAG, "无法成为前台服务，放弃本次截屏授权")
            stopSelf()
            return START_NOT_STICKY
        }

        val resultCode = intent?.getIntExtra(EXTRA_RESULT_CODE, Int.MIN_VALUE) ?: Int.MIN_VALUE
        @Suppress("DEPRECATION")
        val data = intent?.getParcelableExtra<Intent>(EXTRA_RESULT_DATA)
        if (resultCode == Int.MIN_VALUE || data == null) {
            // 系统重建服务（START_NOT_STICKY 下不该发生，防御一下）
            Log.w(TAG, "缺少授权结果，服务无事可做")
            stopSelf()
            return START_NOT_STICKY
        }

        // 2. 换 token。此刻前台服务已在跑，检查能过。
        val ok = ScreenCaptureManager.adoptProjection(applicationContext, resultCode, data)
        if (!ok) {
            Log.w(TAG, "换取 MediaProjection 失败，停止服务")
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        Log.i(TAG, "前台服务销毁，释放 MediaProjection")
        // 只放 token，不要再回头 stopService（否则与 release() 互相递归）
        ScreenCaptureManager.detachProjection()
        super.onDestroy()
    }

    private fun promoteToForeground(): Boolean {
        return try {
            val notification = buildNotification()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(
                    NOTIFICATION_ID,
                    notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION,
                )
            } else {
                // API 24–28：系统还没有类型要求，两参版本即可
                startForeground(NOTIFICATION_ID, notification)
            }
            true
        } catch (t: Throwable) {
            // 少数 ROM 在缺通知权限 / 后台启动受限时会抛；这里返回 false 让调用方
            // 放弃本次授权，而不是留一个「服务在跑但换不到 token」的僵尸状态。
            Log.w(TAG, "startForeground 失败", t)
            false
        }
    }

    private fun buildNotification(): Notification {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "屏幕感知",
                NotificationManager.IMPORTANCE_MIN,
            ).apply {
                description = "让桌宠偶尔看一眼屏幕，主动关心你在做什么"
                setShowBadge(false)
            }
            val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(channel)
        }

        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }

        // 小图标必须能解析，否则部分 ROM 会直接抛异常
        val icon = if (applicationInfo.icon != 0) {
            applicationInfo.icon
        } else {
            android.R.drawable.stat_notify_sync
        }

        return builder
            .setContentTitle("屏幕感知已开启")
            .setContentText("桌宠会偶尔看一眼屏幕；可从状态栏随时停止")
            .setSmallIcon(icon)
            .setOngoing(true)
            .build()
    }

    companion object {
        const val EXTRA_RESULT_CODE = "lingchat.screen.result_code"
        const val EXTRA_RESULT_DATA = "lingchat.screen.result_data"

        /**
         * 启动本服务并传入授权结果。
         *
         * 调用方是 `MainActivity.onActivityResult`（用户刚点了「立即开始」）。
         * 失败返回 false，表示连前台服务都起不来 —— 后面必然换不到 token。
         */
        fun start(context: Context, resultCode: Int, data: Intent): Boolean {
            return try {
                val intent = Intent(context, ScreenCaptureService::class.java)
                    .putExtra(EXTRA_RESULT_CODE, resultCode)
                    .putExtra(EXTRA_RESULT_DATA, data)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
                true
            } catch (t: Throwable) {
                Log.w(TAG, "启动屏幕截取前台服务失败", t)
                false
            }
        }

        fun stop(context: Context) {
            try {
                context.stopService(Intent(context, ScreenCaptureService::class.java))
            } catch (t: Throwable) {
                Log.w(TAG, "停止屏幕截取前台服务失败（可忽略）", t)
            }
        }
    }
}
