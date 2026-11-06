import { extensionAssets } from "@lingchat/dlc-ui/build";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
import vue from "@vitejs/plugin-vue";
import VueDevTools from "vite-plugin-vue-devtools";
import path from "path";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// 主菜单立绘在构建产物中带 hash，index.html 写不死路径。
// 构建收尾时按产物文件名注入 preload，让 LCP 图片请求与入口脚本并行发起。
function preloadMainMenuImage(): Plugin {
  return {
    name: "preload-main-menu-image",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler(html, ctx) {
        const asset = Object.values(ctx.bundle ?? {}).find(
          (item) => item.type === "asset" && /(^|\/)alona-[\w-]+\.webp$/.test(item.fileName),
        );
        if (!asset) return html;
        const tag = `<link rel="preload" as="image" href="/${asset.fileName}" fetchpriority="high" />`;
        return html.replace("</head>", `  ${tag}\n  </head>`);
      },
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(), VueDevTools(), tailwindcss(), extensionAssets(), preloadMainMenuImage()],

  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
      "@lingchat/host": path.resolve(import.meta.dirname, "./src"),
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
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching some files
      ignored: ["**/src-tauri/**", "**/.venv/**", "**/target/**", "**/data/**"],
    },
  },
}));
