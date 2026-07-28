package com.noiq.lingchat.floatingpet

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import org.json.JSONObject

/**
 * 悬浮桌宠前台服务。
 *
 * Action 协议（由 FloatingPetPlugin 通过 startService 触发）：
 *   - ACTION_SHOW       extras: scale (Float, optional)
 *   - ACTION_HIDE       移除 View，Service 仍在运行
 *   - ACTION_STOP       移除 View + stopForeground + stopSelf
 *   - ACTION_UPDATE     extras: payload (JSONObject 字符串)
 *
 * 启动顺序：startForeground() -> attach -> show。Hide 后服务保留以便快速重显。
 */
class FloatingPetService : Service() {

    private var controller: FloatingPetWindowController? = null
    private var notifHelper: PetNotificationHelper? = null
    private val prefs by lazy { SharedPrefs.create(this) }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        notifHelper = PetNotificationHelper(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // startForeground 必须在 onStartCommand 内尽早调用（5s 内），否则 ANR / crash。
        startForegroundCompat()
        val action = intent?.action ?: ACTION_SHOW
        when (action) {
            ACTION_SHOW -> {
                val scale = intent?.getFloatExtra(EXTRA_SCALE, 0f) ?: 0f
                ensureController().attach(if (scale > 0f) scale else 1.0f)
            }
            ACTION_HIDE -> {
                controller?.detach()
                controller = null
            }
            ACTION_STOP -> {
                controller?.detach()
                controller = null
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    stopForeground(STOP_FOREGROUND_REMOVE)
                } else {
                    @Suppress("DEPRECATION")
                    stopForeground(true)
                }
                stopSelf()
            }
            ACTION_UPDATE -> {
                val json = intent?.getStringExtra(EXTRA_PAYLOAD_JSON)
                val state = json?.let { PetState.fromJson(JSONObject(it)) }
                if (state != null) controller?.applyState(state)
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        controller?.detach()
        controller = null
        super.onDestroy()
    }

    private fun startForegroundCompat() {
        val notif = notifHelper?.build() ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            // Android 14+ 需要明确类型，且调用顺序敏感
            startForeground(
                PetNotificationHelper.NOTIFICATION_ID,
                notif,
                android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
            )
        } else {
            startForeground(PetNotificationHelper.NOTIFICATION_ID, notif)
        }
    }

    private fun ensureController(): FloatingPetWindowController {
        controller?.let { return it }
        val c = FloatingPetWindowController(this, prefs) { event ->
            FloatingPetPlugin.emitEvent(event)
        }
        controller = c
        return c
    }

    companion object {
        const val ACTION_SHOW = "com.noiq.lingchat.floatingpet.SHOW"
        const val ACTION_HIDE = "com.noiq.lingchat.floatingpet.HIDE"
        const val ACTION_STOP = "com.noiq.lingchat.floatingpet.STOP"
        const val ACTION_UPDATE = "com.noiq.lingchat.floatingpet.UPDATE"

        const val EXTRA_SCALE = "scale"
        const val EXTRA_PAYLOAD_JSON = "payload_json"

        fun showIntent(ctx: Context, scale: Float?): Intent =
            Intent(ctx, FloatingPetService::class.java).apply {
                action = ACTION_SHOW
                if (scale != null) putExtra(EXTRA_SCALE, scale)
            }

        fun hideIntent(ctx: Context): Intent =
            Intent(ctx, FloatingPetService::class.java).apply { action = ACTION_HIDE }

        fun stopIntent(ctx: Context): Intent =
            Intent(ctx, FloatingPetService::class.java).apply { action = ACTION_STOP }

        fun updateIntent(ctx: Context, payloadJson: String): Intent =
            Intent(ctx, FloatingPetService::class.java).apply {
                action = ACTION_UPDATE
                putExtra(EXTRA_PAYLOAD_JSON, payloadJson)
            }
    }
}
