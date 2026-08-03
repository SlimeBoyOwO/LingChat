import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import VueDevTools from 'vite-plugin-vue-devtools'
import path from 'path'
import tailwindcss from '@tailwindcss/vite'

const host = process.env.TAURI_DEV_HOST

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(), VueDevTools(), tailwindcss()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching some files
      // data/ 是运行时数据目录（角色立绘、音乐等），导入/播放时会临时占用文件，
      // 被 vite watch 会触发 EBUSY 崩溃，必须排除。
      ignored: ['**/src-tauri/**', '**/.venv/**', '**/target/**', '**/data/**'],
    },
  },

  // 依赖优化配置
  optimizeDeps: {
    exclude: ['src-tauri/*'],
    entries: [
      'src/*'
    ],
  },
}))