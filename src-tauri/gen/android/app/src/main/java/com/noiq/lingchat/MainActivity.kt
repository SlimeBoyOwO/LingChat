package com.noiq.lingchat

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // 启用 Edge-to-Edge 沉浸式显示
    // enableEdgeToEdge() 会自动：
    // 1. 设置状态栏/导航栏透明
    // 2. 根据 DayNight 主题自动切换系统栏图标颜色
    // 3. 安装 WindowInsets 监听器，将安全区域传递给 WebView
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
