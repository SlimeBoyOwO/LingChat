package com.noiq.lingchat.floatingpet

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import java.io.File
import java.io.FileInputStream
import java.net.URLDecoder
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import androidx.core.app.NotificationCompat

/**
 * 鍓嶅彴鏈嶅姟閫氱煡鏋勫缓銆侫ndroid 8+ 寮哄埗瑕佹眰 channel銆?
 *
 * - smallIcon 蹇呴』鏄?monochrome vector锛堝崟鑹茬伆搴曪級锛岀敤浜?ic_floating_pet 鍗犱綅銆?
 * - largeIcon 鏉ヨ嚜瑙掕壊绔嬬粯缂╁埌 96x96 鐨?Bitmap缂撳瓨锛岄€氳繃 setAvatar() 寮傛垚绛夊緟瑙gelineIcon 鍚庣敤 readyListener 鍥炶皟閫氱煡 Service 閲嶅缓 notification銆?
 */
class PetNotificationHelper(private val context: Context) {

    fun interface OnLargeIconReadyListener {
        fun onLargeIconReady()
    }

    @Volatile
    private var pendingPath: String? = null
    @Volatile
    private var currentPath: String? = null
    @Volatile
    private var currentName: String? = null
    @Volatile
    private var largeIconBitmap: Bitmap? = null

    private val ioExecutor: ExecutorService = Executors.newSingleThreadExecutor { r ->
        Thread(r, "FloatingPet-IconDecoder").apply { isDaemon = true }
    }
    private val mainHandler = Handler(Looper.getMainLooper())
    private var readyListener: OnLargeIconReadyListener? = null

    init {
        ensureChannel()
    }

    fun setOnLargeIconReadyListener(listener: OnLargeIconReadyListener?) {
        readyListener = listener
    }

    /**
     * 璁剧疆褰撳墠瑙掕壊涓庣珛缁樻潵婧愶細
     * - 濡傛灉 name 涓嶄负绌哄垯鏇存柊 title 瀛楁锛?
     * - 濡傛灉 path 鏀瑰彉鍒欏紓姝ヨВ鐮佸苟缂╂斁鍒?NOTIFICATION_ICON_SIZE x NOTIFICATION_ICON_SIZE 缂撳瓨涓哄ぇ鍥炬爣銆?
     */
    fun setAvatar(path: String?, name: String?) {
        if (!name.isNullOrEmpty() && name != currentName) {
            currentName = name
        }
        if (path.isNullOrEmpty() || path == currentPath) return
        pendingPath = path
        ioExecutor.execute { decodeAvatar(path) }
    }

    private fun decodeAvatar(path: String) {
        if (path != pendingPath) return
        val resolved = resolveAvatarPath(path)
        val decoded = if (resolved != null) {
            runCatching {
                val opts = BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.ARGB_8888 }
                FileInputStream(resolved).use { BitmapFactory.decodeStream(it, null, opts) }
            }.getOrNull()
        } else null
        if (path != pendingPath) {
            decoded?.recycle()
            return
        }
        val target = decoded?.let {
            Bitmap.createScaledBitmap(it, NOTIFICATION_ICON_SIZE, NOTIFICATION_ICON_SIZE, true)
        }
        val previous = largeIconBitmap
        largeIconBitmap = target
        if (previous != null && previous !== target) {
            previous.recycle()
        }
        currentPath = path
        mainHandler.post { readyListener?.onLargeIconReady() }
    }

    private fun resolveAvatarPath(url: String): File? {
        return runCatching {
            when {
                url.startsWith("asset://") -> {
                    val raw = url.removePrefix("asset://localhost/").removePrefix("asset://")
                    File(URLDecoder.decode(raw, "UTF-8"))
                }
                url.startsWith("file://") -> {
                    File(URLDecoder.decode(Uri.parse(url).path ?: return@runCatching null, "UTF-8"))
                }
                else -> File(url)
            }
        }.getOrNull()
    }

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val mgr = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (mgr.getNotificationChannel(CHANNEL_ID) == null) {
                val ch = NotificationChannel(
                    CHANNEL_ID,
                    "鎮诞妗屽疇",
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = "淇濇寔鎮诞妗屽疇杩愯鎵€蹇呴渶"
                    setShowBadge(false)
                }
                mgr.createNotificationChannel(ch)
            }
        }
    }

    fun build(): Notification {
        val hideIntent = Intent(context, FloatingPetService::class.java).apply {
            action = FloatingPetService.ACTION_HIDE
        }
        val hidePi = PendingIntent.getService(
            context, 1, hideIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val stopIntent = Intent(context, FloatingPetService::class.java).apply {
            action = FloatingPetService.ACTION_STOP
        }
        val stopPi = PendingIntent.getService(
            context, 2, stopIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val iconRes = try {
            com.noiq.lingchat.floatingpet.R.drawable.ic_floating_pet
        } catch (_: Throwable) {
            android.R.drawable.ic_dialog_info
        }
        val title = if (!currentName.isNullOrEmpty()) {
            "LingChat 鎮诞妗屽疇 路 $currentName"
        } else {
            "LingChat 鎮诞妗屽疇"
        }
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(iconRes)
            .setContentTitle(title)
            .setContentText("妗屽疇姝ｅ湪杩愯 路 闀挎寜澶村儚鎵撳紑鑿滃崟")
            .setOngoing(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .addAction(0, "闅愯棌", hidePi)
            .addAction(0, "鍋滄", stopPi)
        largeIconBitmap?.let { builder.setLargeIcon(it) }
        return builder.build()
    }

    companion object {
        const val NOTIFICATION_ID = 8801
        const val CHANNEL_ID = "floating_pet_service"
        private const val NOTIFICATION_ICON_SIZE = 96
    }
}