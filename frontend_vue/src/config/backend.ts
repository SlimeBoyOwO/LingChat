/**
 * 后端地址配置管理模块
 *
 * 配置优先级（开发环境）：
 * 1. URL参数：?backend=http://host:port
 * 2. 环境变量：VITE_BACKEND_BIND_ADDR 和 VITE_BACKEND_PORT
 * 3. 默认值：localhost:8764
 *
 * 生产环境：直接使用当前页面 origin
 */

// 判断是否为开发环境（基于Vite注入的环境变量）
function isDevelopment(): boolean {
  return import.meta.env.DEV === true || import.meta.env.MODE === 'development'
}

// 开发环境：支持URL参数和环境变量
function getDevBackendBaseUrl(): string {
  // 1. 检查URL参数
  const urlParams = new URLSearchParams(window.location.search)
  const backendUrl = urlParams.get('backend')

  if (backendUrl) {
    try {
      const url = new URL(backendUrl)
      return url.origin // 返回完整的 origin（协议+主机+端口）
    } catch (e) {
      console.warn('Invalid backend URL parameter:', backendUrl)
    }
  }

  // 2. 使用环境变量（开发时从Vite注入）
  const host = import.meta.env.VITE_BACKEND_BIND_ADDR || 'localhost'
  const port = import.meta.env.VITE_BACKEND_PORT || '8764'
  const protocol = window.location.protocol.includes('https') ? 'https' : 'http'

  return `${protocol}://${host}:${port}`
}

// 生产环境：直接使用当前页面 origin
function getProdBackendBaseUrl(): string {
  // 直接使用浏览器当前页面的基础URL
  // 例如：https://localhost:8394/menu → https://localhost:8394
  return window.location.origin
}

// 统一的API基础URL获取
export function getApiBaseUrl(): string {
  const baseUrl = isDevelopment() ? getDevBackendBaseUrl() : getProdBackendBaseUrl()
  return `${baseUrl}/api`
}

// 统一的WebSocket URL获取
export function getWebSocketUrl(): string {
  const baseUrl = isDevelopment() ? getDevBackendBaseUrl() : getProdBackendBaseUrl()
  // 替换协议：http → ws, https → wss
  const wsBaseUrl = baseUrl.replace(/^http/, 'ws')
  return `${wsBaseUrl}/ws`
}

// 获取后端基础URL（不带路径）
export function getBackendBaseUrl(): string {
  return isDevelopment() ? getDevBackendBaseUrl() : getProdBackendBaseUrl()
}

// 导出环境判断函数
export { isDevelopment }
