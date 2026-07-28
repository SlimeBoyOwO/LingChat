package com.noiq.lingchat.floatingpet

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.RadialGradient
import android.graphics.Paint
import android.graphics.Shader
import android.graphics.Rect
import android.animation.ObjectAnimator
import android.animation.ValueAnimator
import android.view.animation.AccelerateDecelerateInterpolator
import android.net.Uri
import android.util.AttributeSet
import android.util.Log
import android.view.MotionEvent
import android.view.View
import java.io.File
import java.io.FileInputStream
import java.net.URLDecoder

/**
 * 桌宠头像视图。
 *
 * - 可交互：onTouchEvent 由 PetGestureHandler 消费，分发 tap/double_tap/long_press/drag/pinch。
 * - 默认圆形渲染 avatar 图；图缺失时显示占位圆 + 角色首字。
 * - avatarUrl 支持 `asset://localhost/<path>` 与纯本地路径两种形式。
 */
class FloatingPetAvatarView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0,
) : View(context, attrs, defStyleAttr) {

    interface Listener {
        fun onEvent(event: PetEvent)
    }

    var listener: Listener? = null
    var avatarScale: Float = 1.0f
        set(value) {
            field = value.coerceIn(0.5f, 2.0f)
            requestLayout()
            invalidate()
        }

    private var avatarBitmap: Bitmap? = null
    private var displayName: String = ""
    private var dragging: Boolean = false
    private var breathingScale: Float = 1.0f
    private var longPressScale: Float = 1.0f
    private var longPressGlow: Float = 0.0f
    private var breathingAnimator: ObjectAnimator? = null
    private var longPressAnimator: ValueAnimator? = null

    private val bitmapPaint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG)
    private val glowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.FILL
    }
    private val placeholderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#80FFFFFF")
        style = Paint.Style.FILL
    }
    private val placeholderStroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#FFFFFFFF")
        style = Paint.Style.STROKE
        strokeWidth = 2f * resources.displayMetrics.density
    }
    private val initialPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        textAlign = Paint.Align.CENTER
        textSize = 36f * resources.displayMetrics.scaledDensity
    }

    private val gestureHandler = PetGestureHandler(resources.displayMetrics.density, object : PetGestureHandler.Callbacks {
        override fun onTap() {
            emit(PetEvent.Tap(rawXInWindow(), rawYInWindow()))
        }
        override fun onDoubleTap() {
            emit(PetEvent.DoubleTap(rawXInWindow(), rawYInWindow()))
        }
        override fun onLongPress() {
            startLongPressHighlight()
            emit(PetEvent.LongPress(rawXInWindow(), rawYInWindow()))
        }
        override fun onDragStart(x: Float, y: Float) { dragging = true }
        override fun onDragMove(x: Float, y: Float) {
            (parent as? android.view.ViewGroup)?.let { /* no-op, controller handles via window params */ }
        }
        override fun onDragEnd(x: Float, y: Float) {
            dragging = false
            emit(PetEvent.DragEnd(x, y))
        }
        override fun onPinch(scale: Float) {
            emit(PetEvent.Pinch(scale))
        }
    })

    init {
        isClickable = true
        isFocusable = false
    }

    fun applyState(state: PetState) {
        var changed = false
        state.scale?.let {
            val s = it.coerceIn(0.5, 2.0).toFloat()
            if (s != avatarScale) { avatarScale = s; changed = true }
        }
        state.characterName?.let {
            if (it != displayName) { displayName = it; changed = true }
        }
        state.avatarUrl?.let { url ->
            loadAvatarAsync(url)
        }
        state.audioPlaying?.let { playing ->
            if (playing && breathingAnimator == null) startBreathing()
            else if (!playing && breathingAnimator != null) stopBreathing()
        }
        if (changed) invalidate()
    }

    private fun startBreathing() {
        breathingAnimator?.cancel()
        breathingAnimator = ObjectAnimator.ofFloat(this, "breathingScale", 1.0f, 1.03f).apply {
            duration = 800L
            repeatCount = ValueAnimator.INFINITE
            repeatMode = ValueAnimator.REVERSE
            interpolator = AccelerateDecelerateInterpolator()
            start()
        }
    }

    private fun stopBreathing() {
        breathingAnimator?.cancel()
        breathingAnimator = null
        breathingScale = 1.0f
        invalidate()
    }

    private fun startLongPressHighlight() {
        longPressAnimator?.cancel()
        val startScale = longPressScale
        val startGlow = longPressGlow
        longPressAnimator = ValueAnimator.ofFloat(0f, 1f).apply {
            duration = 160L
            interpolator = AccelerateDecelerateInterpolator()
            addUpdateListener { va ->
                val t = va.animatedFraction
                longPressScale = startScale + (1.05f - startScale) * t
                longPressGlow = startGlow + (0.35f - startGlow) * t
                invalidate()
            }
            start()
        }
    }

    private fun stopLongPressHighlight() {
        longPressAnimator?.cancel()
        val startScale = longPressScale
        val startGlow = longPressGlow
        longPressAnimator = ValueAnimator.ofFloat(0f, 1f).apply {
            duration = 180L
            interpolator = AccelerateDecelerateInterpolator()
            addUpdateListener { va ->
                val t = va.animatedFraction
                longPressScale = startScale + (1.0f - startScale) * t
                longPressGlow = startGlow + (0.0f - startGlow) * t
                invalidate()
            }
            start()
        }
    }

    /** ObjectAnimator 反射写入的 setter。 */
    @Suppress("unused")
    fun setBreathingScale(v: Float) {
        breathingScale = v
        invalidate()
    }

    private fun loadAvatarAsync(url: String) {
        val resolved = resolveAvatarPath(url) ?: run {
            Log.w(TAG, "avatar url not resolvable: $url")
            avatarBitmap = null
            invalidate()
            return
        }
        post {
            val bmp = runCatching {
                val opts = BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.ARGB_8888 }
                FileInputStream(resolved).use { BitmapFactory.decodeStream(it, null, opts) }
            }.getOrNull()
            avatarBitmap = bmp
            invalidate()
        }
    }

    /**
     * 接受 `asset://localhost/<path>`、`file://...` 或纯绝对路径。
     */
    private fun resolveAvatarPath(url: String): File? {
        return runCatching {
            when {
                url.startsWith("asset://") -> {
                    val path = url.removePrefix("asset://localhost/").removePrefix("asset://")
                    File(URLDecoder.decode(path, "UTF-8"))
                }
                url.startsWith("file://") -> {
                    File(URLDecoder.decode(Uri.parse(url).path ?: return@runCatching null, "UTF-8"))
                }
                else -> File(url)
            }
        }.getOrNull()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val base = BASE_SIZE_DP * resources.displayMetrics.density
        val size = (base * avatarScale).toInt()
        setMeasuredDimension(size, size)
    }

    override fun onDraw(canvas: Canvas) {
        val w = width.toFloat()
        val h = height.toFloat()
        val s = breathingScale
        val lpScale = longPressScale
        if (s != 1.0f) {
            canvas.save()
            canvas.scale(s, s, w / 2f, h / 2f)
        }
        super.onDraw(canvas)
        if (lpScale != 1.0f) {
            canvas.save()
            canvas.scale(lpScale, lpScale, w / 2f, h / 2f)
        }
        val bmp = avatarBitmap
        if (bmp != null) {
            val rect = Rect(0, 0, w.toInt(), h.toInt())
            canvas.drawBitmap(bmp, null, rect, bitmapPaint)
        } else {
            canvas.drawCircle(w / 2f, h / 2f, w / 2f, placeholderPaint)
            canvas.drawCircle(w / 2f, h / 2f, w / 2f, placeholderStroke)
            val initial = displayName.firstOrNull()?.uppercase() ?: "?"
            val yOffset = (initialPaint.descent() + initialPaint.ascent()) / 2f
            canvas.drawText(initial, w / 2f, h / 2f - yOffset, initialPaint)
        }
        if (longPressGlow > 0f) {
            val cx = w / 2f
            val cy = h / 2f
            val radius = (maxOf(w, h) * 0.6f) * lpScale
            glowPaint.shader = RadialGradient(
                cx, cy, radius,
                intArrayOf(
                    Color.argb((255 * longPressGlow).toInt(), 255, 255, 255),
                    Color.argb(0, 255, 255, 255),
                ),
                floatArrayOf(0f, 1f),
                Shader.TileMode.CLAMP,
            )
            canvas.drawCircle(cx, cy, radius, glowPaint)
        }
        if (lpScale != 1.0f) canvas.restore()
        if (s != 1.0f) canvas.restore()
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (dragging && event.actionMasked == MotionEvent.ACTION_MOVE) {
            return true
        }
        gestureHandler.onTouchEvent(event)
        return true
    }

    private fun emit(event: PetEvent) {
        listener?.onEvent(event)
    }

    private fun rawXInWindow(): Float {
        val loc = IntArray(2).also { getLocationOnScreen(it) }
        return loc[0].toFloat() + width / 2f
    }

    private fun rawYInWindow(): Float {
        val loc = IntArray(2).also { getLocationOnScreen(it) }
        return loc[1].toFloat() + height / 2f
    }

    fun resetGesture() {
        gestureHandler.reset()
        dragging = false
        if (longPressScale != 1.0f || longPressGlow > 0f) {
            stopLongPressHighlight()
        }
    }

    companion object {
        private const val TAG = "FloatingPetAvatarView"
        private const val BASE_SIZE_DP = 120f
    }
}
