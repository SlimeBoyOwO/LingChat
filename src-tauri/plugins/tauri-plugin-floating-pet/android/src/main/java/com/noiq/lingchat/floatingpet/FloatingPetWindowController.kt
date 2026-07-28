package com.noiq.lingchat.floatingpet

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.view.Gravity
import android.view.MenuItem
import android.view.WindowManager
import android.view.WindowMetrics
import android.widget.PopupMenu
import androidx.annotation.RequiresApi

/**
 * 持有 WindowManager 与两个 View（avatar / dialogue），负责 add / remove / update。
 *
 * - 外层 LayoutParams 标志：FLAG_NOT_FOCUSABLE | FLAG_NOT_TOUCH_MODAL | FLAG_LAYOUT_NO_LIMITS
 * - AvatarView 可点击消费触摸；DialogueView 透传触摸到下层 App（isClickable=false）。
 * - 拖动结束自动贴边（snap_to_edge），并写入 SharedPrefs。
 * - 长按头像弹出 PopupMenu（5 秒自动关闭）。
 */
class FloatingPetWindowController(
    private val context: Context,
    private val prefs: SharedPrefs,
    private val eventListener: (PetEvent) -> Unit,
) {
    private val windowManager: WindowManager =
        context.getSystemService(Context.WINDOW_SERVICE) as WindowManager

    private var avatarView: FloatingPetAvatarView? = null
    private var dialogueView: FloatingPetDialogueView? = null
    private var avatarParams: WindowManager.LayoutParams? = null
    private var dialogueParams: WindowManager.LayoutParams? = null
    private var popupMenu: PopupMenu? = null
    private val mainHandler = Handler(Looper.getMainLooper())
    private val menuAutoDismiss = Runnable {
        popupMenu?.dismiss()
        popupMenu = null
        avatarView?.resetGesture()
    }

    private var screenW: Int = 0
    private var screenH: Int = 0

    @SuppressLint("ClickableViewAccessibility")
    fun attach(initialScale: Float) {
        if (avatarView != null) return
        computeScreenSize()
        val scale = initialScale.takeIf { it > 0f } ?: prefs.lastScale
        val avatar = FloatingPetAvatarView(context).apply {
            avatarScale = scale
            listener = object : FloatingPetAvatarView.Listener {
                override fun onEvent(event: PetEvent) {
                    handleEvent(event)
                }
            }
        }
        val dialogue = FloatingPetDialogueView(context)

        avatarParams = baseParams().apply {
            gravity = Gravity.TOP or Gravity.START
            val (x, y) = resolveStartPosition(scale)
            this.x = x
            this.y = y
            width = avatarSizePx(scale)
            height = avatarSizePx(scale)
        }
        dialogueParams = baseParams().apply {
            gravity = Gravity.TOP or Gravity.START
            this.x = (avatarParams!!.x).coerceAtLeast(0)
            this.y = (avatarParams!!.y - dp(72f)).coerceAtLeast(0)
            width = WindowManager.LayoutParams.WRAP_CONTENT
            height = WindowManager.LayoutParams.WRAP_CONTENT
        }

        try {
            windowManager.addView(avatar, avatarParams)
            windowManager.addView(dialogue, dialogueParams)
            avatarView = avatar
            dialogueView = dialogue
        } catch (e: WindowManager.BadTokenException) {
            avatarView = null
            dialogueView = null
        }
    }

    fun detach() {
        avatarView?.let { runCatching { windowManager.removeView(it) } }
        dialogueView?.let { runCatching { windowManager.removeView(it) } }
        avatarView = null
        dialogueView = null
        avatarParams = null
        dialogueParams = null
    }

    fun applyState(state: PetState) {
        val avatar = avatarView ?: return
        avatar.applyState(state)
        state.scale?.let {
            avatarParams?.let { p ->
                p.width = avatarSizePx(avatar.avatarScale)
                p.height = avatarSizePx(avatar.avatarScale)
                runCatching { windowManager.updateViewLayout(avatar, p) }
            }
            prefs.lastScale = avatar.avatarScale
        }
        dialogueView?.applyState(state)
        dialogueView?.let { d ->
            d.measure(
                android.view.View.MeasureSpec.makeMeasureSpec(dp(220f).toInt(), android.view.View.MeasureSpec.AT_MOST),
                android.view.View.MeasureSpec.makeMeasureSpec(0, android.view.View.MeasureSpec.UNSPECIFIED),
            )
            val w = d.measuredWidth
            val h = d.measuredHeight
            dialogueParams?.let { p ->
                val ax = avatarParams?.x ?: 0
                val ay = avatarParams?.y ?: 0
                val aw = avatarParams?.width ?: 0
                val dx = (ax + (aw - w) / 2).coerceIn(0, screenW - w)
                val dy = (ay - h - dp(8f)).toInt().coerceAtLeast(0)
                p.x = dx
                p.y = dy
                p.width = w
                p.height = h
                runCatching { windowManager.updateViewLayout(d, p) }
            }
        }
    }

    private fun handleEvent(event: PetEvent) {
        when (event) {
            is PetEvent.LongPress -> showLongPressMenu()
            is PetEvent.DragEnd -> {
                val av = avatarParams ?: return
                val view = avatarView ?: return
                val w = av.width
                snapToEdge(event.x.toInt(), event.y.toInt())
                eventListener(PetEvent.DragEnd((av.x + w / 2f), (av.y + view.height / 2f)))
            }
            is PetEvent.Pinch -> {
                val cur = (avatarView?.avatarScale ?: 1.0f)
                val next = (cur * event.scale).coerceIn(0.5f, 2.0f)
                avatarView?.avatarScale = next
                applyState(PetState(scale = next.toDouble()))
                eventListener(event)
            }
            else -> eventListener(event)
        }
    }

    /**
     * 长按头像时弹出 PopupMenu。5 秒无操作自动关闭。
     *   - 隐藏：仅移除 View（Service 仍在），由 JS 端决定何时停止。
     *   - 打开 WebView：emit bring_to_front 事件，JS 把窗口拉回前台。
     *   - 完全停止服务：startService(stopIntent) 让 Service 自我销毁。
     */
    private fun showLongPressMenu() {
        val av = avatarView ?: return
        mainHandler.removeCallbacks(menuAutoDismiss)
        popupMenu?.dismiss()
        val menuRes = try {
            com.noiq.lingchat.floatingpet.R.menu.floating_pet_menu
        } catch (_: Throwable) {
            return
        }
        val pm = PopupMenu(context, av, Gravity.TOP or Gravity.START)
        pm.menuInflater.inflate(menuRes, pm.menu)
        pm.setOnMenuItemClickListener { item: MenuItem ->
            when (item.itemId) {
                com.noiq.lingchat.floatingpet.R.id.fp_menu_hide -> {
                    detach()
                    avatarView?.resetGesture()
                    true
                }
                com.noiq.lingchat.floatingpet.R.id.fp_menu_toggle -> {
                    eventListener(PetEvent.BringToFront)
                    true
                }
                com.noiq.lingchat.floatingpet.R.id.fp_menu_stop -> {
                    runCatching {
                        context.startService(FloatingPetService.stopIntent(context))
                    }
                    true
                }
                else -> false
            }
        }
        pm.setOnDismissListener {
            mainHandler.removeCallbacks(menuAutoDismiss)
            popupMenu = null
        }
        pm.show()
        popupMenu = pm
        mainHandler.postDelayed(menuAutoDismiss, 5_000L)
    }

    private fun snapToEdge(rawX: Int, rawY: Int) {
        val p = avatarParams ?: return
        val w = p.width
        val view = avatarView ?: return
        val h = view.height
        val mid = screenW / 2
        p.x = if (rawX < mid) {
            0
        } else {
            (screenW - w).coerceAtLeast(0)
        }
        p.y = rawY.coerceIn(0, screenH - h)
        runCatching { windowManager.updateViewLayout(avatarView, p) }
        prefs.lastX = p.x
        prefs.lastY = p.y

        val dp = dialogueParams
        val dv = dialogueView
        if (dp != null && dv != null) {
            dv.measure(
                android.view.View.MeasureSpec.makeMeasureSpec(dp(220f).toInt(), android.view.View.MeasureSpec.AT_MOST),
                android.view.View.MeasureSpec.makeMeasureSpec(0, android.view.View.MeasureSpec.UNSPECIFIED),
            )
            val dw = dv.measuredWidth
            val dh = dv.measuredHeight
            dp.x = (p.x + (w - dw) / 2).coerceIn(0, screenW - dw)
            dp.y = (p.y - dh - dp(8f)).toInt().coerceAtLeast(0)
            dp.width = dw
            dp.height = dh
            runCatching { windowManager.updateViewLayout(dv, dp) }
        }
    }

    private fun resolveStartPosition(scale: Float): Pair<Int, Int> {
        val px = avatarSizePx(scale)
        val savedX = prefs.lastX
        val savedY = prefs.lastY
        if (savedX >= 0 && savedY >= 0) {
            return Pair(
                savedX.coerceIn(0, (screenW - px).coerceAtLeast(0)),
                savedY.coerceIn(0, (screenH - px).coerceAtLeast(0)),
            )
        }
        val x = (screenW - px - dp(16f)).toInt().coerceAtLeast(0)
        val y = (screenH - px - dp(96f)).toInt().coerceAtLeast(0)
        return Pair(x, y)
    }

    private fun avatarSizePx(scale: Float): Int {
        return (120f * scale * context.resources.displayMetrics.density).toInt()
    }

    private fun dp(v: Float): Float = v * context.resources.displayMetrics.density

    private fun baseParams(): WindowManager.LayoutParams {
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }
        val p = WindowManager.LayoutParams(
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            type,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                or WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT,
        )
        runCatching {
            p.windowAnimations = com.noiq.lingchat.floatingpet.R.style.FloatingPetWindowAnimations
        }
        return p
    }

    @Suppress("DEPRECATION")
    private fun computeScreenSize() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val metrics: WindowMetrics = windowManager.currentWindowMetrics
            val bounds = metrics.bounds
            screenW = bounds.width()
            screenH = bounds.height()
        } else {
            val dm = DisplayMetrics()
            @Suppress("DEPRECATION")
            windowManager.defaultDisplay.getRealMetrics(dm)
            screenW = dm.widthPixels
            screenH = dm.heightPixels
        }
    }

    fun isAttached(): Boolean = avatarView != null

    fun resetGesture() {
        avatarView?.resetGesture()
    }

    @Suppress("unused")
    @RequiresApi(Build.VERSION_CODES.R)
    private fun currentWindowMetrics() = windowManager.currentWindowMetrics
}
