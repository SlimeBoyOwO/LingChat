package com.noiq.lingchat.floatingpet

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat

/**
 * 前台服务通知构建。Android 8+ 强制要求 channel。
 */
class PetNotificationHelper(private val context: Context) {

    init {
        ensureChannel()
    }

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val mgr = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (mgr.getNotificationChannel(CHANNEL_ID) == null) {
                val ch = NotificationChannel(
                    CHANNEL_ID,
                    "悬浮桌宠",
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = "保持悬浮桌宠运行所必需"
                    setShowBadge(false)
                }
                mgr.createNotificationChannel(ch)
            }
        }
    }

    fun build(): Notification {
        val stopIntent = Intent(context, FloatingPetService::class.java).apply {
            action = FloatingPetService.ACTION_STOP
        }
        val stopPi = PendingIntent.getService(
            context, 0, stopIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val iconRes = try {
            com.noiq.lingchat.floatingpet.R.drawable.ic_floating_pet
        } catch (_: Throwable) {
            android.R.drawable.ic_dialog_info
        }
        return NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(iconRes)
            .setContentTitle("LingChat 悬浮桌宠")
            .setContentText("桌宠正在运行 · 点击此处返回")
            .setOngoing(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .addAction(0, "停止", stopPi)
            .build()
    }

    companion object {
        const val NOTIFICATION_ID = 8801
        const val CHANNEL_ID = "floating_pet_service"
    }
}
