package com.noiq.lingchat

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat

/**
 * 模型下载期间常驻的前台服务。
 *
 * Android 在息屏/后台一段时间后会冻结普通进程，几百 MB 的下载很容易因此中断。
 * Rust 侧 `utils::android_keepalive` 在下载开始时通过 JNI 调用 [start]，结束时
 * 调用 [stop]；服务只负责挂一个常驻通知，把进程提升到前台优先级。
 */
class DownloadKeepAliveService : Service() {
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val label = intent?.getStringExtra(EXTRA_LABEL)
        acquireWakeLock()
        startForeground(NOTIFICATION_ID, buildNotification(label))
        // 系统重启服务时没有下载上下文，交给系统按需重启没有意义。
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        releaseWakeLock()
        super.onDestroy()
        stopForeground(Service.STOP_FOREGROUND_REMOVE)
    }

    /**
     * 息屏后 CPU 仍可能休眠，网络 IO 会跟着停摆；前台服务只保证进程优先级，
     * 这里再加一个 PARTIAL_WAKE_LOCK，等服务结束时释放。
     */
    private fun acquireWakeLock() {
        if (wakeLock != null) return
        val power = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = power
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "LingChat:modelDownload")
            .apply {
                setReferenceCounted(false)
                acquire()
            }
    }

    private fun releaseWakeLock() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
    }

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.download_keepalive_channel),
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = getString(R.string.download_keepalive_description)
            setSound(null, null)
            enableVibration(false)
        }
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(label: String?): Notification {
        val title = if (label.isNullOrBlank()) {
            getString(R.string.download_keepalive_title)
        } else {
            getString(R.string.download_keepalive_title_with_label, label)
        }

        val contentIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentTitle(title)
            .setContentText(getString(R.string.download_keepalive_description))
            .setContentIntent(contentIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .build()
    }

    companion object {
        private const val CHANNEL_ID = "lingchat_model_download"
        private const val NOTIFICATION_ID = 0x4C43 // "LC"
        private const val EXTRA_LABEL = "label"

        /** 由 Rust 通过 JNI 调用。 */
        @JvmStatic
        fun start(context: Context, label: String) {
            val intent = Intent(context, DownloadKeepAliveService::class.java)
                .putExtra(EXTRA_LABEL, label)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /** 由 Rust 通过 JNI 调用；重复调用无副作用。 */
        @JvmStatic
        fun stop(context: Context) {
            context.stopService(Intent(context, DownloadKeepAliveService::class.java))
        }
    }
}
