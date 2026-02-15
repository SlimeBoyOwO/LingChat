import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { connectWebSocket } from './api/websocket'
import { initializeEventProcessors } from './core/events'
import { getWebSocketUrl } from './config/backend'

import App from './App.vue'
import './assets/styles/base.css'
import './assets/styles/variables.css'

import './api/websocket/handlers/script-handler'

import router from './router' // './router/index.js' 的简写

// 导入日志转发插件
import logForwarderPlugin from './plugins/logForwarder'

const app = createApp(App)

connectWebSocket(getWebSocketUrl())

initializeEventProcessors()

app.use(createPinia())
app.use(router)
app.use(logForwarderPlugin, {
  // 插件配置
  appName: 'LingChat',
  version: '1.0.0',
})
app.mount('#app')
