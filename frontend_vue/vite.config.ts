import { fileURLToPath, URL } from 'node:url'
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueJsx from '@vitejs/plugin-vue-jsx'
import vueDevTools from 'vite-plugin-vue-devtools'
import tailwindcss from '@tailwindcss/vite'
import fs from 'fs'
import path from 'path'

// 读取项目根目录的.env文件，参考后端load_env.py实现
function loadRootEnv() {
  const rootDir = path.resolve(__dirname, '..')
  const envPath = path.join(rootDir, '.env')

  if (fs.existsSync(envPath)) {
    const envContent = fs.readFileSync(envPath, 'utf-8')
    const env: Record<string, string> = {}

    const lines = envContent.split('\n')

    lines.forEach((line, index) => {
      // 移除行首尾空白
      const trimmed = line.trim()

      // 跳过空行和注释行
      if (!trimmed || trimmed.startsWith('#')) {
        return
      }

      // 跳过标记行（如 BEGIN/END）
      if ((trimmed.includes('BEGIN') || trimmed.includes('END')) && !trimmed.includes('=')) {
        return
      }

      // 分割键值对
      if (trimmed.includes('=')) {
        const equalsIndex = trimmed.indexOf('=')
        const key = trimmed.substring(0, equalsIndex).trim()
        let value = trimmed.substring(equalsIndex + 1).trim()

        const originalValue = value

        // 去除值后面的注释（#后面的内容）但保留引号内的#字符
        let commentIndex = -1
        let inQuotes = false
        let quoteChar = ''

        for (let i = 0; i < value.length; i++) {
          const char = value[i]
          if ((char === '"' || char === "'") && (i === 0 || value[i-1] !== '\\')) {
            if (!inQuotes) {
              inQuotes = true
              quoteChar = char
            } else if (char === quoteChar) {
              inQuotes = false
              quoteChar = ''
            }
          } else if (char === '#' && !inQuotes) {
            commentIndex = i
            break
          }
        }

        if (commentIndex !== -1) {
          value = value.substring(0, commentIndex).trim()
        }

        // 去除值两端的引号（单引号或双引号）
        if ((value.startsWith('"') && value.endsWith('"')) ||
            (value.startsWith("'") && value.endsWith("'"))) {
          value = value.substring(1, value.length - 1)
        }

        // 再次trim，确保没有多余空格
        value = value.trim()

        // 只记录重要的环境变量以减少日志噪音
        const importantKeys = ['BACKEND_BIND_ADDR', 'BACKEND_PORT', 'FRONTEND_BIND_ADDR', 'FRONTEND_PORT']
        env[key] = value
      }
    })
    return env
  }

  return {}
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  // 加载前端环境变量
  const frontendEnv = loadEnv(mode, process.cwd(), '')

  // 加载项目根目录环境变量
  const rootEnv = loadRootEnv()

  // 合并环境变量，根目录环境变量优先级更高
  const mergedEnv = {
    ...frontendEnv,
    ...rootEnv,
  }

  // 获取后端配置
  const backendHost = mergedEnv.BACKEND_BIND_ADDR || 'localhost'
  const backendPort = mergedEnv.BACKEND_PORT || '8765'

  const httpTarget = `http://${backendHost}:${backendPort}`
  const wsTarget = `ws://${backendHost}:${backendPort}`

  return {
    plugins: [vue(), vueJsx(), vueDevTools(), tailwindcss()],
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url)),
      },
    },
    server: {
      proxy: {
        // 代理普通 HTTP API 请求
        '/api': {
          target: httpTarget,
          changeOrigin: true,
        },
        // 代理 WebSocket 连接
        '/ws': {
          target: wsTarget,
          changeOrigin: true,
          ws: true, // 启用 WebSocket 代理
        },
      },
    },
    // 将环境变量注入到客户端
    define: {
      'import.meta.env.VITE_BACKEND_BIND_ADDR': JSON.stringify(backendHost),
      'import.meta.env.VITE_BACKEND_PORT': JSON.stringify(backendPort),
    },
  }
})
