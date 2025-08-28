import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { spawn } from "child_process";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    {
      name: 'Backend',
      configureServer(server) {
        const process = spawn('backend.cmd', [], {
          stdio: 'pipe',
          shell: true
        })
        process.stdout.on('data', (data) => {
          console.log(data.toString().trim())
        })
        process.stderr.on('data', (data) => {
          console.error(data.toString().trim())
        })
        process.on('error', (error) => {
          console.error(`[后端][错误]:${error}`)
        })
        server.httpServer?.on('close', () => {
          process.kill()
        })
      },
    }
  ],
  resolve: {
    alias: {
      "@": "/src",
    },
  },
  server: {
    proxy: {
      // 代理普通 HTTP API 请求
      "/api": {
        target: "http://localhost:8765",
        changeOrigin: true
      },
      // 代理 WebSocket 连接
      "/ws": {
        target: "ws://localhost:8765", // WebSocket 地址
        changeOrigin: true,
        ws: true // 启用 WebSocket 代理
      },
    },
  },
});
