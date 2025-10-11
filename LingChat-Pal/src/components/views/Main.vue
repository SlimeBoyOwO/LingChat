<template>
  <div id="app">
    <!-- 空心圆容器 -->
    <div class="avatar-container">
      <!-- 空心圆 -->
      <div class="avatar-frame" data-tauri-drag-region>
        <StarField
          ref="starfieldRef"
          :enabled="starfieldEnabled"
          :star-count="starCount"
          :scroll-speed="scrollSpeed"
          :colors="starColors"
          class="star-field"
        />
        <div class="image-container">
          <img :src="imageUrl" alt="头像" class="avatar-image" />
        </div>
      </div>
    </div>

    <button class="open-window-btn" @click="openSecondWindow">诺一 钦灵</button>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import StarField from "../particles/StarField.vue";

// 使用本地图片路径或网络图片URL
const imageUrl = ref("src/assets/avatar.png"); // 请替换为你的图片路径

// 星空效果控制
const starfieldEnabled = ref(true);
const starCount = ref(120);
const scrollSpeed = ref(0.1);
const starColors = ref([
  "rgb(173, 216, 230)",
  "rgb(176, 224, 230)",
  "rgb(241, 141, 252)",
  "rgb(176, 230, 224)",
  "rgb(173, 230, 216)",
]);

const openSecondWindow = async () => {
  try {
    // 根据运行环境选择正确的 URL：
    // - 开发模式 (Vite dev server)：使用当前 location.origin + #/second（例如 http://localhost:5173/#/second）
    // - 打包到 Tauri：使用 index.html#/second（Tauri 会加载 dist 的 index.html）
    const isDev = Boolean((import.meta as any).env?.DEV);
    const targetUrl = isDev
      ? `${window.location.origin}#/second`
      : "index.html#/second";

    const webview = new WebviewWindow("settings", {
      url: targetUrl,
      title: "设置",
      width: 1200,
      height: 800,
      resizable: true,
      shadow: false,
      decorations: false,
      transparent: true,
      alwaysOnTop: false,
      visible: true,
    });

    webview.once("tauri://created", () => {
      console.log("第二个窗口创建成功");
    });

    webview.once("tauri://error", (e) => {
      console.error("创建第二个窗口失败:", e);
    });
  } catch (error) {
    console.error("打开第二个窗口时出错:", error);
  }
};
</script>

<style scoped>
/* 容器用于居中空心圆 */
.avatar-container {
  position: absolute;
  margin: auto;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
}

/* 空心圆样式 */
.avatar-frame {
  width: 210px; /* 圆环外径 */
  height: 210px;

  padding: 2px;
  border-radius: 50%; /* 关键：使其变为圆形 */
  background: transparent; /* 内部透明 */
  box-sizing: border-box;
  display: flex;
  justify-content: center;
  align-items: center;

  background: rgba(0, 0, 0, 0.01);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 2px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  transition: border-color 0.2s, box-shadow 0.2s;

  -webkit-app-region: drag;
}

.avatar-frame::before {
  content: "";

  width: 208px;
  height: 208px;

  background-color: transparent;
  background-image: conic-gradient(
    transparent,
    var(--accent-color),
    transparent 50%
  );

  border-radius: 50%;

  position: absolute;
  padding: 3px;
  z-index: -2;

  -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;

  animation: rotate 5s linear infinite;
}

.avatar-frame::after {
  content: "";
  position: absolute;
  width: 225px;
  height: 225px;
  border-radius: 50%;
  background: transparent;
  border: 1px groove rgba(255, 255, 255, 0.1);
  animation: rotate-reverse 20s linear infinite;
}

.star-field {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  overflow: hidden;
  z-index: -1; /* 确保星空在最底层 */
}

@property --angle {
  syntax: "<angle>";
  initial-value: 0deg;
  inherits: false;
}

--glass-effect {
  /*磨砂玻璃效果*/
  background: rgba(255, 255, 255, 0.011);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  transition: border-color 0.2s, box-shadow 0.2s;
}

@keyframes rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* 图片容器，稍小于空心圆 */
.image-container {
  width: 100%; /* 200px - 2*15px 边框 */
  height: 100%;
  border-radius: 50%;
  overflow: hidden; /* 关键：确保图片被裁剪为圆形 */
  background: transparent;
}

/* 图片样式 */
.avatar-image {
  width: 100%;
  height: 100%;
  object-fit: cover; /* 关键：保持图片比例并填满容器 */
  background-size: contain;
  background-position: center center;
  background-repeat: no-repeat;
  z-index: 1;
  transition: background-image 0.2s ease-in-out;
  transform-origin: center 0%;
  animation: breathing 4s infinite; /* 呼吸动画 */
  overflow: hidden;
}

/* 呼吸动画关键帧 */
@keyframes breathing {
  0%,
  100% {
    transform: scale(1); /* 正常大小 */
  }
  50% {
    transform: scale(1.01); /* 轻微放大 */
  }
}

/*动画发光效果*/
@keyframes glow {
  0% {
    box-shadow: #329ea3 0px 0px 5px, #329ea3 0px 0px 10px, #329ea3 0px 0px 15px;
  }
  50% {
    box-shadow: #329ea3 0px 0px 10px, #329ea3 0px 0px 40px, #329ea3 0px 0px 60px;
  }
  100% {
    box-shadow: #329ea3 0px 0px 5px, #329ea3 0px 0px 10px, #329ea3 0px 0px 15px;
  }
}

@keyframes rotate-border {
  to {
    --angle: 360deg;
  }
}

.open-window-btn {
  position: absolute;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 16px;
  background: rgba(50, 158, 163, 0.8);
  color: white;
  border: none;
  border-radius: 15px;
  cursor: pointer;
  z-index: 10;
}

.open-window-btn:hover {
  background: rgba(50, 158, 163, 1);
}
</style>
