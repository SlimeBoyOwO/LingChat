package com.noiq.floatingpet

/**
 * 桌宠「说话」的极简出口 —— 现在只剩一个跨 Activity 共享的状态位。
 *
 * ## 2026-10-17：从系统 Toast 改成页面内 toast
 *
 * 早先这里有个 `toastDenied()`，用户拒绝授权时用 Android `Toast` 弹一句
 * 「哇呜呜，口是心非的人类，居然不允许我的请求」。理由是那两刻 WebView
 * 可能不可靠（刚 resume / 停在桌宠布局）。
 *
 * 但系统 Toast 是灰底黑字的系统气泡，跟 App 的视觉完全脱节，也不受主题控制。
 * 而实际上这两次拒绝都发生在**页面可用**的时刻，所以改成：
 *
 * ```text
 * MainActivity.notifyPermissionDenied("overlay" | "screen")
 *   → evaluateJavascript 派发 CustomEvent('pet-permission-denied')
 *   → 前端监听 → uiStore.showWarning() 弹应用内 toast
 * ```
 *
 * 台词挪到了前端 i18n，与其它文案一起维护。
 *
 * ## 为什么这个 object 还留着
 *
 * `awaitingOverlayPermission` 要跨 `FloatingPetPlugin`（置位方）和
 * `MainActivity`（消费方）共享，而这两个类**不同模块、不同包**。
 * 一个顶层 `object` 是最小耦合的载体。
 */
object PetVoice {

    /**
     * 是否正在等「悬浮窗权限」的结果。
     *
     * 系统设置页不会回传结果（`ACTION_MANAGE_OVERLAY_PERMISSION` 不是
     * `startActivityForResult` 语义），只能靠「回来后有没有真的拿到权限」判断。
     *
     * - 置位方：`FloatingPetPlugin.requestPermission`
     * - 消费方：`MainActivity.onResume`
     */
    @Volatile
    var awaitingOverlayPermission: Boolean = false
}
