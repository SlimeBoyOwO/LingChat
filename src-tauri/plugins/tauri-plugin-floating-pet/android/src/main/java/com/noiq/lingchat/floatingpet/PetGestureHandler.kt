package com.noiq.lingchat.floatingpet

import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.MotionEvent
import kotlin.math.abs
import kotlin.math.hypot

/**
 * 单手指势状态机（IDLE / WAITING_TAP_OR_LONGPRESS / ACTIVE / DRAGGING）。
 *
 * 设计要点：
 * - 单击头像不会触发拖动；必须先长按 1000ms 进入 ACTIVE 后才能拖动。
 * - 200ms 内、位移 < 12dp 才计为单击；两次单击计为双击。
 * - 双指捏合单独处理（任意时刻进入 / 离开 POINTER_DOWN），与单指状态机并行。
 */
class PetGestureHandler(
    private val density: Float,
    private val callbacks: Callbacks,
) {
    interface Callbacks {
        fun onTap()
        fun onDoubleTap()
        fun onLongPress()
        fun onDragStart(x: Float, y: Float)
        fun onDragMove(x: Float, y: Float)
        fun onDragEnd(x: Float, y: Float)
        fun onPinch(scale: Float)
    }

    private enum class State { IDLE, WAITING, ACTIVE, DRAGGING }

    private var state: State = State.IDLE
    private var downX = 0f
    private var downY = 0f
    private var lastX = 0f
    private var lastY = 0f
    private var downTimeMs = 0L
    private var lastTapTimeMs = 0L
    private var activeAnchorX = 0f
    private var activeAnchorY = 0f

    private val handler = Handler(Looper.getMainLooper())
    private val longPressRunnable = Runnable {
        if (state == State.WAITING) {
            state = State.ACTIVE
            activeAnchorX = downX
            activeAnchorY = downY
            callbacks.onLongPress()
        }
    }

    private var initialPinchDistance = 0f
    private var pinching = false

    private val tapThresholdMs = 200L
    private val longPressMs = 1000L
    private val doubleTapGapMs = 250L
    private val moveThresholdDp = 12f
    private val moveThresholdPx = moveThresholdDp * density

    fun onTouchEvent(ev: MotionEvent): Boolean {
        // 多指处理（捏合）
        if (ev.pointerCount >= 2) {
            return handlePinch(ev)
        }

        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downX = ev.x
                downY = ev.y
                lastX = ev.x
                lastY = ev.y
                downTimeMs = SystemClock.uptimeMillis()
                state = State.WAITING
                handler.removeCallbacks(longPressRunnable)
                handler.postDelayed(longPressRunnable, longPressMs)
                return true
            }

            MotionEvent.ACTION_MOVE -> {
                val dx = ev.x - downX
                val dy = ev.y - downY
                val moved = hypot(dx, dy) > moveThresholdPx

                when (state) {
                    State.WAITING -> {
                        if (moved) {
                            // 在长按触发前移动 -> 取消 WAITING，回到 IDLE（不进入 ACTIVE）
                            handler.removeCallbacks(longPressRunnable)
                            state = State.IDLE
                        }
                    }
                    State.ACTIVE -> {
                        if (moved) {
                            state = State.DRAGGING
                            callbacks.onDragStart(ev.rawX - activeAnchorX, ev.rawY - activeAnchorY)
                        }
                    }
                    State.DRAGGING -> {
                        callbacks.onDragMove(ev.rawX - activeAnchorX, ev.rawY - activeAnchorY)
                    }
                    State.IDLE -> { /* no-op */ }
                }
                lastX = ev.x
                lastY = ev.y
                return true
            }

            MotionEvent.ACTION_UP -> {
                handler.removeCallbacks(longPressRunnable)
                val elapsed = SystemClock.uptimeMillis() - downTimeMs
                val moved = hypot(ev.x - downX, ev.y - downY) > moveThresholdPx

                when (state) {
                    State.WAITING -> {
                        if (elapsed <= tapThresholdMs && !moved) {
                            handleTap()
                        }
                    }
                    State.ACTIVE -> {
                        // 长按后松开但不拖动 = 视为 tap（推进对话）
                        if (!moved) {
                            callbacks.onTap()
                        }
                    }
                    State.DRAGGING -> {
                        callbacks.onDragEnd(ev.rawX - activeAnchorX, ev.rawY - activeAnchorY)
                    }
                    State.IDLE -> { /* no-op */ }
                }
                state = State.IDLE
                lastX = ev.x
                lastY = ev.y
                return true
            }

            MotionEvent.ACTION_CANCEL -> {
                handler.removeCallbacks(longPressRunnable)
                state = State.IDLE
                return true
            }
        }
        return false
    }

    private fun handleTap() {
        val now = SystemClock.uptimeMillis()
        if (now - lastTapTimeMs <= doubleTapGapMs) {
            lastTapTimeMs = 0L
            callbacks.onDoubleTap()
        } else {
            lastTapTimeMs = now
            callbacks.onTap()
        }
    }

    private fun handlePinch(ev: MotionEvent): Boolean {
        when (ev.actionMasked) {
            MotionEvent.ACTION_POINTER_DOWN -> {
                initialPinchDistance = pinchDistance(ev)
                pinching = initialPinchDistance > 0f
                handler.removeCallbacks(longPressRunnable)
                state = State.IDLE
            }
            MotionEvent.ACTION_MOVE -> {
                if (!pinching) return false
                val d = pinchDistance(ev)
                if (d <= 0f || initialPinchDistance <= 0f) return false
                val ratio = d / initialPinchDistance
                callbacks.onPinch(ratio)
            }
            MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                pinching = false
                initialPinchDistance = 0f
            }
        }
        return true
    }

    private fun pinchDistance(ev: MotionEvent): Float {
        if (ev.pointerCount < 2) return 0f
        val dx = ev.getX(0) - ev.getX(1)
        val dy = ev.getY(0) - ev.getY(1)
        return hypot(dx, dy)
    }

    fun reset() {
        handler.removeCallbacks(longPressRunnable)
        state = State.IDLE
        pinching = false
        initialPinchDistance = 0f
    }

    companion object {
        @Suppress("unused")
        private const val TAG = "PetGestureHandler"
        // 暴露给 Kotlin 端的 abs 引用，避免 lint 报 unused
        @Suppress("unused")
        private fun _abs(v: Float) = abs(v)
    }
}
