/**
 * 后端地址配置管理模块
 *
 * 配置策略：
 * - 开发环境：使用当前页面 origin，通过 Vite 代理转发
 * - 生产环境：直接使用当前页面 origin
 * - URL参数支持：?backend=http://host:port
 */

// 判断是否为开发环境（基于Vite注入的环境变量）
function isDevelopment(): boolean {
  return import.meta.env.DEV === true || import.meta.env.MODE === 'development'
}

// 获取基础URL
function getBackendBaseUrl(): string {
  // 检查URL参数（用于特殊调试场景）
  const urlParams = new URLSearchParams(window.location.search)
  const backendUrl = urlParams.get('backend')

  if (backendUrl) {
    try {
      const url = new URL(backendUrl)
      console.log('使用URL参数指定的后端地址:', url.origin)
      return url.origin
    } catch (e) {
      console.warn('Invalid backend URL parameter:', backendUrl)
    }
  }

  return window.location.origin
}

// 统一的API基础URL获取
export function getApiBaseUrl(): string {
  const baseUrl = getBackendBaseUrl()

  // 如果使用的是页面 origin（默认情况），使用相对路径让 Vite 代理处理
  if (baseUrl === window.location.origin) {
    return '/api'
  }

  // 如果指定了外部后端地址
  return `${baseUrl}/api`
}

// 统一的WebSocket URL获取
export function getWebSocketUrl(): string {
  const baseUrl = getBackendBaseUrl()

  // 如果使用的是页面 origin（默认情况），使用相对路径让 Vite 代理处理
  if (baseUrl === window.location.origin) {
    // 协议转换为 ws/wss
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    return `${protocol}//${window.location.host}/ws`
  }

  // 如果指定了外部后端地址
  const wsBaseUrl = baseUrl.replace(/^http/, 'ws')
  return `${wsBaseUrl}/ws`
}

// 导出环境判断函数
export { isDevelopment }
