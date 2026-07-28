package com.noiq.lingchat.floatingpet

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.animation.ValueAnimator
import android.view.animation.LinearInterpolator
import android.util.AttributeSet
import android.view.View

/**
 * 桌宠对话气泡视图。
 *
 * 设计要点：
 * - 不消费触摸（isClickable=false）：触摸直接穿透到下层 App。
 * - 仅绘制文本气泡：圆角矩形 + 指向头像的小尾巴。
 * - 长文本自动截断为 200 字 + "..."。
 */
class FloatingPetDialogueView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0,
) : View(context, attrs, defStyleAttr) {

    private val density = resources.displayMetrics.density
    private var dialogueText: String = ""
    private var isTyping: Boolean = false

    private val bubblePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#E6202020")
        style = Paint.Style.FILL
    }
    private val bubbleStroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#FFFFFFFF")
        style = Paint.Style.STROKE
        strokeWidth = 1f * density
    }
    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        textSize = 14f * resources.displayMetrics.scaledDensity
        textAlign = Paint.Align.LEFT
    }
    private val tailPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#E6202020")
        style = Paint.Style.FILL
    }
    private val bubbleRect = RectF()
    private val tailPath = Path()
    private val padding = 12f * density

    private var breathingAnimator: ValueAnimator? = null

    init {
        isClickable = false
        isFocusable = false
    }

    fun applyState(state: PetState) {
        state.dialogueText?.let {
            dialogueText = it
        }
        state.dialogueTyping?.let {
            isTyping = it
        }
        state.audioPlaying?.let { playing ->
            if (playing && breathingAnimator == null) startBreathing()
            else if (!playing && breathingAnimator != null) stopBreathing()
        }
        requestLayout()
        invalidate()
    }

    private fun startBreathing() {
        breathingAnimator?.cancel()
        breathingAnimator = ValueAnimator.ofFloat(0.75f, 1.0f).apply {
            duration = 900
            repeatCount = ValueAnimator.INFINITE
            repeatMode = ValueAnimator.REVERSE
            interpolator = LinearInterpolator()
            addUpdateListener {
                alpha = it.animatedValue as Float
            }
            start()
        }
    }

    private fun stopBreathing() {
        breathingAnimator?.cancel()
        breathingAnimator = null
        alpha = 1.0f
    }

    override fun onDetachedFromWindow() {
        stopBreathing()
        super.onDetachedFromWindow()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val maxTextWidth = (MAX_WIDTH_DP * density).toInt()
        val text = truncated()
        val textWidth = textPaint.measureText(text).toInt() + (padding * 2).toInt()
        val w = textWidth.coerceAtMost(maxTextWidth)
        val lineCount = wrapLines(text, w - (padding * 2).toInt()).size
        val lineHeight = (textPaint.fontSpacing).toInt()
        val h = lineHeight * lineCount + (padding * 2).toInt()
        setMeasuredDimension(w, h)
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val w = width.toFloat()
        val h = height.toFloat()
        val radius = 10f * density

        bubbleRect.set(0f, 0f, w, h - radius)
        canvas.drawRoundRect(bubbleRect, radius, radius, bubblePaint)
        canvas.drawRoundRect(bubbleRect, radius, radius, bubbleStroke)

        // 尾巴指向头像（向左下）
        tailPath.reset()
        tailPath.moveTo(w / 2f - 8f * density, h - radius)
        tailPath.lineTo(w / 2f, h.toFloat())
        tailPath.lineTo(w / 2f + 8f * density, h - radius)
        tailPath.close()
        canvas.drawPath(tailPath, tailPaint)

        val text = truncated()
        val lines = wrapLines(text, w - (padding * 2).toInt())
        var y = padding + textPaint.textSize
        for (line in lines) {
            canvas.drawText(line, padding, y, textPaint)
            y += textPaint.fontSpacing
        }
    }

    private fun truncated(): String {
        if (dialogueText.isEmpty()) return if (isTyping) "..." else ""
        return if (dialogueText.length > MAX_CHARS) {
            dialogueText.substring(0, MAX_CHARS - 1) + "..."
        } else {
            dialogueText
        }
    }

    private fun wrapLines(text: String, maxWidth: Int): List<String> {
        if (text.isEmpty()) return emptyList()
        val out = mutableListOf<String>()
        val sb = StringBuilder()
        for (ch in text) {
            sb.append(ch)
            if (textPaint.measureText(sb.toString()) > maxWidth) {
                sb.deleteCharAt(sb.length - 1)
                out.add(sb.toString())
                sb.clear()
                sb.append(ch)
            }
        }
        if (sb.isNotEmpty()) out.add(sb.toString())
        return out
    }

    companion object {
        private const val MAX_WIDTH_DP = 220f
        private const val MAX_CHARS = 200
    }
}
