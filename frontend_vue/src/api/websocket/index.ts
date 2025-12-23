import { ref } from 'vue'
import type { WebSocketHandler } from '../../types'

const socket = ref<WebSocket | null>(null)
const handlers = new Map<string, WebSocketHandler>()
const reconnectAttempts = ref(0)
const maxReconnectAttempts = 5
const reconnectDelay = 3000

export const connectWebSocket = (url: string) => {
  socket.value = new WebSocket(url)

  socket.value.onopen = () => {
    console.log('WebSocket connected')
    reconnectAttempts.value = 0
  }

  socket.value.onmessage = (event) => {
    try {
      const message = JSON.parse(event.data)
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
  return false
}

export const sendWebSocketChatMessage = (type: string, content: string) => {
  if (socket.value?.readyState === WebSocket.OPEN) {
    socket.value.send(JSON.stringify({ type, content }))
    return true
  }
  return false
}

export const closeWebSocket = () => {
  if (socket.value) {
    socket.value.close()
    socket.value = null
  }
}
