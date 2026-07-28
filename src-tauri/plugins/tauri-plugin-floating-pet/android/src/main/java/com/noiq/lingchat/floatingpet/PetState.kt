package com.noiq.lingchat.floatingpet

/**
 * 从 WebView 推送到 Kotlin 的桌宠状态。
 *
 * 字段可空：未提供的字段视为不更新。
 */
data class PetState(
    val characterId: String? = null,
    val characterName: String? = null,
    val avatarUrl: String? = null,
    val expression: String? = null,
    val dialogueText: String? = null,
    val dialogueTyping: Boolean? = null,
    val audioPlaying: Boolean? = null,
    val scale: Double? = null,
    val volume: Int? = null,
    val backgroundEffect: String? = null,
    val visible: Boolean? = null,
) {
    companion object {
        /**
         * WebView 通过 update_pet_state 传入的 camelCase JSON 映射成此结构。
         */
        fun fromJson(o: org.json.JSONObject?): PetState? {
            if (o == null) return null
            val character = o.optJSONObject("character")
            val dialogue = o.optJSONObject("dialogue")
            return PetState(
                characterId = character?.optString("id")?.takeIf { it.isNotEmpty() },
                characterName = character?.optString("name")?.takeIf { it.isNotEmpty() },
                avatarUrl = character?.optString("avatarUrl")?.takeIf { it.isNotEmpty() },
                expression = character?.optString("expression")?.takeIf { it.isNotEmpty() },
                dialogueText = dialogue?.optString("text")?.takeIf { it.isNotEmpty() },
                dialogueTyping = dialogue?.optBoolean("isTyping"),
                audioPlaying = dialogue?.optBoolean("audioPlaying"),
                scale = if (o.has("scale") && !o.isNull("scale")) o.optDouble("scale") else null,
                volume = if (o.has("volume") && !o.isNull("volume")) o.optInt("volume") else null,
                backgroundEffect = o.optString("backgroundEffect")?.takeIf { it.isNotEmpty() },
                visible = if (o.has("visible") && !o.isNull("visible")) o.optBoolean("visible") else null,
            )
        }
    }
}

/**
 * Kotlin -> WebView 的事件负载（统一以 JSONObject 表示，Tauri 端负责序列化为 { type, payload }）。
 */
sealed class PetEvent(val type: String) {
    data class Tap(val x: Float, val y: Float) : PetEvent("tap")
    data class DoubleTap(val x: Float, val y: Float) : PetEvent("double_tap")
    data class LongPress(val x: Float, val y: Float) : PetEvent("long_press")
    data class DragEnd(val x: Float, val y: Float) : PetEvent("drag_end")
    data class Pinch(val scale: Float) : PetEvent("pinch")
    /** 菜单点击：用户请求把 WebView 拉到前台。 */
    data object BringToFront : PetEvent("bring_to_front")

    fun toJson(): org.json.JSONObject {
        val o = org.json.JSONObject()
        o.put("type", type)
        when (this) {
            is Tap -> o.put("payload", org.json.JSONObject().put("x", x.toDouble()).put("y", y.toDouble()))
            is DoubleTap -> o.put("payload", org.json.JSONObject().put("x", x.toDouble()).put("y", y.toDouble()))
            is LongPress -> o.put("payload", org.json.JSONObject().put("x", x.toDouble()).put("y", y.toDouble()))
            is DragEnd -> o.put("payload", org.json.JSONObject().put("x", x.toDouble()).put("y", y.toDouble()))
            is Pinch -> o.put("payload", org.json.JSONObject().put("scale", scale.toDouble()))
            is BringToFront -> { /* 无 payload */ }
        }
        return o
    }
}
