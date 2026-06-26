import { ref } from 'vue'
import type { WebSocketHandler } from '../../types'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useGameStore } from '../../stores/modules/game'

const socket = ref<WebSocket | null>(null)
const handlers = new Map<string, WebSocketHandler>()
const reconnectAttempts = ref(0)
const maxReconnectAttempts = 5
const reconnectDelay = 3000

// 心跳配置
const heartbeatInterval = 25000 // 每 25s 发送一次心跳，略短于服务端 ping 间隔 (30s)
let heartbeatTimer: ReturnType<typeof setInterval> | null = null

// 连接状态追踪
const connectionReady = ref(false) // WebSocket 是否已经成功连接过
const initializationTime = Date.now() // 页面初始化时间
const gracePeriodMs = 3000 // 初始化宽限期（毫秒）

// 判断是否应该显示连接错误
const shouldShowConnectionError = (): boolean => {
  // 如果已经成功连接过，则应该显示错误
  if (connectionReady.value) {
    return true
  }
  // 如果还在初始化宽限期内，不显示错误
  if (Date.now() - initializationTime < gracePeriodMs) {
    console.log('WebSocket 仍在初始化宽限期内，暂不显示连接错误')
    return false
  }
  // 宽限期已过但从未连接成功，应该显示错误
  return true
}

// 显示连接错误并重置状态的辅助函数
const handleConnectionError = (errorMessage: string = '无法连接到服务器') => {
  // 检查是否应该显示错误
  if (!shouldShowConnectionError()) {
    return
  }

  const uiStore = useUIStore()
  const gameStore = useGameStore()

  // 显示错误通知
  uiStore.showError({
    errorCode: 'network_error',
    message: errorMessage,
  })

  // 重置游戏状态
  gameStore.currentStatus = 'input'
  gameStore.currentLine = ''
  console.log('游戏状态已重置为: input (WebSocket断开)')
}

// 启动心跳：定期发送 ping 以保持连接活跃
const startHeartbeat = () => {
  stopHeartbeat() // 避免重复启动
  heartbeatTimer = setInterval(() => {
    if (socket.value?.readyState === WebSocket.OPEN) {
      socket.value.send(JSON.stringify({ type: 'ping' }))
    }
  }, heartbeatInterval)
}

// 停止心跳
const stopHeartbeat = () => {
  if (heartbeatTimer !== null) {
    clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }
}

export const connectWebSocket = (url: string) => {
  socket.value = new WebSocket(url)

  socket.value.onopen = () => {
    console.log('WebSocket connected')
    reconnectAttempts.value = 0
    connectionReady.value = true // 标记连接已就绪
    // 连接成功后启动心跳
    startHeartbeat()
  }

  socket.value.onmessage = (event) => {
    try {
      const message = JSON.parse(event.data)
      // pong 是心跳响应，静默忽略
      if (message.type === 'pong') {
        return
      }
      const handler = handlers.get(message.type)
      if (handler) {
        handler(message)
      } else {
        console.warn(`No handler for message type: ${message.type}`)
      }
    } catch (error) {
      console.error('Error parsing WebSocket message:', error)
    }
  }

  socket.value.onclose = () => {
    console.log('WebSocket disconnected')
    // 断开时停止心跳
    stopHeartbeat()
    if (reconnectAttempts.value < maxReconnectAttempts) {
      setTimeout(() => {
        reconnectAttempts.value++
        connectWebSocket(url)
      }, reconnectDelay)
    }
  }

  socket.value.onerror = (error) => {
    console.error('WebSocket error:', error)
  }
}

export const registerHandler = (type: string, handler: WebSocketHandler) => {
  handlers.set(type, handler)
}

export const unregisterHandler = (type: string) => {
  handlers.delete(type)
}

export const sendWebSocketMessage = (type: string, data: any) => {
  if (socket.value?.readyState === WebSocket.OPEN) {
    socket.value.send(JSON.stringify({ type, data }))
    return true
  }
  // 发送失败时显示错误
  handleConnectionError('后端服务未连接，请启动后端服务')
  return false
}

export const sendWebSocketChatMessage = (
  type: string,
  content: string,
  extra: Record<string, unknown> = {},
) => {
  if (socket.value?.readyState === WebSocket.OPEN) {
    socket.value.send(JSON.stringify({ type, content, ...extra }))
    return true
  }
  // 发送失败时显示错误
  handleConnectionError('后端服务未连接，请启动后端服务')
  return false
}

export const closeWebSocket = () => {
  stopHeartbeat()
  if (socket.value) {
    socket.value.close()
    socket.value = null
  }
}
