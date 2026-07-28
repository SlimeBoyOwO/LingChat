package com.noiq.lingchat.floatingpet

import android.content.Context
import android.content.SharedPreferences

/**
 * 桌宠轻量持久化：位置 / 缩放 / 权限说明已展示。
 */
class SharedPrefs private constructor(private val sp: SharedPreferences) {

    var lastX: Int
        get() = sp.getInt(KEY_X, -1)
        set(v) { sp.edit().putInt(KEY_X, v).apply() }

    var lastY: Int
        get() = sp.getInt(KEY_Y, -1)
        set(v) { sp.edit().putInt(KEY_Y, v).apply() }

    var lastScale: Float
        get() = sp.getFloat(KEY_SCALE, 1.0f)
        set(v) { sp.edit().putFloat(KEY_SCALE, v).apply() }

    var permissionExplanationShown: Boolean
        get() = sp.getBoolean(KEY_EXPL_SHOWN, false)
        set(v) { sp.edit().putBoolean(KEY_EXPL_SHOWN, v).apply() }

    companion object {
        private const val FILE = "floating_pet_prefs"
        private const val KEY_X = "last_x"
        private const val KEY_Y = "last_y"
        private const val KEY_SCALE = "last_scale"
        private const val KEY_EXPL_SHOWN = "expl_shown"

        fun create(ctx: Context): SharedPrefs {
            return SharedPrefs(ctx.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE))
        }
    }
}
