package com.noiq.floatingpet

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.util.Log

private const val TAG = "FloatingPet"
private const val CHANNEL_ID = "lingchat_pet"
private const val NOTIFICATION_ID = 0x504554 // "PET"

/**
 * 桌宠前台服务——保证悬浮窗在 App 退到后台后不被系统回收。
 *
 * ## 为什么必须有它
 *
 * 悬浮窗里的 WebView 由**本 App 进程**承载。App 退到后台后进程降为
 * 后台优先级，内存紧张时会被系统直接杀掉，悬浮窗随之消失——用户看到
 * 的就是「桌宠自己没了」。
 *
 * 前台服务把进程提升为前台优先级，是 Android 上唯一能长期驻留的正当
 * 手段（后台 Service 在 8.0+ 会被限制，JobScheduler 无法维持实时渲染）。
 *
 * ## 与「WebView 被暂停」的区别
 *
 * 这是两个独立问题，别混淆：
 * - **被暂停**：进程还活着，但 `WryActivity.onPause()` 调了
 *   `mWebView.onPause()`，桌宠静止不动。由插件侧 `onPause/onResume`
 *   重新唤醒解决（见 `FloatingPetPlugin.resumePetWebView`）。
 * - **被回收**：进程被杀，悬浮窗整个消失。由本服务解决。
 *
 * ## 通知
 *
 * Android 8.0+ 前台服务必须常驻一条通知，这是系统强制的（无法隐藏）。
 * 这里用 `IMPORTANCE_MIN` 让它尽量不打扰：不响铃、不弹横幅、不显示角标。
 */
class PetForegroundService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        try {
            startForeground(NOTIFICATION_ID, buildNotification())
        } catch (e: Exception) {
            // 少数 ROM 在缺通知权限时会抛异常；服务仍会被启动，
            // 只是没有前台通知，此时退化为普通后台服务（聊胜于无）。
            Log.w(TAG, "启动前台通知失败（可忽略）", e)
        }
        // START_STICKY：被系统杀掉后尝试重建，尽量维持桌宠存活
        return START_STICKY
    }

    private fun buildNotification(): Notification {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "桌宠运行中",
                NotificationManager.IMPORTANCE_MIN
            ).apply {
                description = "保持桌宠悬浮窗在后台运行"
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

        // 小图标必须能解析，否则部分 ROM 会直接抛异常。
        // applicationInfo.icon 在极少数情况下为 0，因此给一个系统兜底图标。
        val icon = if (applicationInfo.icon != 0) {
            applicationInfo.icon
        } else {
            android.R.drawable.stat_notify_sync
        }

        return builder
            .setContentTitle("桌宠正在运行")
            .setContentText("轻点悬浮窗展开，双击收回")
            .setSmallIcon(icon)
            .setOngoing(true)
            .build()
    }

    companion object {
        /** 启动前台服务（桌宠进入悬浮窗时调用）。 */
        fun start(context: Context) {
            try {
                val intent = Intent(context, PetForegroundService::class.java)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
            } catch (e: Exception) {
                // 前台服务启动受限（如后台启动限制）时不致命：桌宠仍能用，
                // 只是退到后台后更容易被系统回收。
                Log.w(TAG, "启动桌宠前台服务失败（可忽略）", e)
            }
        }

        /** 停止前台服务（桌宠收回 App 时调用）。 */
        fun stop(context: Context) {
            try {
                context.stopService(Intent(context, PetForegroundService::class.java))
            } catch (e: Exception) {
                Log.w(TAG, "停止桌宠前台服务失败（可忽略）", e)
            }
        }
    }
}
