import { createApp } from 'vue'
import pinia from './stores'
import { connectWebSocket } from './api/websocket'
import { initializeEventProcessors } from './core/events'
import { getWebSocketUrl } from './config/backend'
import 'element-plus/dist/index.css'
import ElementPlus from 'element-plus'

import App from './App.vue'
import './assets/styles/base.css'
import './assets/styles/variables.css'

import './api/websocket/handlers/script-handler'
import './api/websocket/handlers/adventure-handler'

import router from './router' // './router/index.js' 的简写

// 导入日志转发插件
import logForwarderPlugin from './plugins/logForwarder'

// 导入性能检测
import {
  initializePerformanceDetection,
  getRecommendedSettings,
} from './utils/devicePerformance'
import { useSettingsStore } from './stores/modules/settings'

// TODO: 清理旧版本 localStorage 残留数据（v2.x 之前的独立存储格式，以后此逻辑可以删除）
const LEGACY_KEYS = [
  'lingchat-bubble-volume',
  'lingchat-text-speed',
  'lingchat-background-volume',
  'lingchat-character-volume',
  'lingchat-achievement-volume',
  'lingchat_character_folder',
]
LEGACY_KEYS.forEach((key) => {
  if (localStorage.getItem(key) !== null) {
    localStorage.removeItem(key)
    console.log(`[Cleanup] 已删除旧版本残留: ${key}`)
  }
})

const app = createApp(App)

// 使用动态 WebSocket URL
const wsUrl = getWebSocketUrl()
console.log('WebSocket 连接地址:', wsUrl)
connectWebSocket(wsUrl)

initializeEventProcessors()

app.use(pinia)

// 性能检测并应用设置（必须在 pinia 初始化后执行）
async function initPerformanceSettings() {
  const { profile, isFirstDetection } = await initializePerformanceDetection()

  // 只有首次检测才应用设置
  if (isFirstDetection) {
    const settingsStore = useSettingsStore()
    const recommendedSettings = getRecommendedSettings(profile)

    // 应用推荐的显示设置
    settingsStore.updateDisplay(recommendedSettings)

    console.log('[性能检测] 已应用推荐设置:', recommendedSettings)
  }
}

// 执行性能检测（异步，不阻塞应用启动）
initPerformanceSettings()

app.use(router)
app.use(ElementPlus)
app.use(logForwarderPlugin, {
  // 插件配置
  appName: 'LingChat',
  version: '1.0.0',
})
app.mount('#app')
