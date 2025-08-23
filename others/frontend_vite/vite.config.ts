import { defineConfig } from "vite";

export default defineConfig({
  server: {
    proxy: {
      // 代理普通 HTTP API 请求
      "/api": {
        target: "http://localhost:8765",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, "/api"), // 可调整路径
      },
      // 代理 WebSocket 连接
      "/ws": {
        target: "ws://localhost:8765", // WebSocket 地址
        changeOrigin: true,
        ws: true, // 启用 WebSocket 代理
        rewrite: (path) => path.replace(/^\/ws/, "/ws"), // 可调整路径
      },
    },
  },
});
