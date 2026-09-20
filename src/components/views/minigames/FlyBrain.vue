<template>
  <div ref="host" class="fly-brain-page"></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { mountFlyBrain, type FlyBrainController } from "./fly-brain/game.js";
import template from "./fly-brain/template.html?raw";
import styles from "./fly-brain/style.css?raw";

const router = useRouter();
const host = ref<HTMLDivElement | null>(null);
const lifetime = new AbortController();
let controller: FlyBrainController | undefined;

onMounted(async () => {
  if (!host.value) return;
  // Shadow DOM 隔离牧场场景样式，避免全局皮肤影响 WebGL 画布与控件
  const root = host.value.attachShadow({ mode: "open" });
  const style = document.createElement("style");
  style.textContent = styles;
  root.innerHTML = template;
  root.prepend(style);
  try {
    // 幂等：首次进入会加载脑模型并校准（约 1 秒内）
    await invoke("fly_brain_enter");
  } catch {
    // 后端未就绪也照常挂载，状态轮询会自动跳过失败帧
  }
  if (lifetime.signal.aborted) return;
  controller = await mountFlyBrain(root, {
    signal: lifetime.signal,
    onExit: () => {
      void router.push("/mini-games");
    },
  });
});

onBeforeUnmount(() => {
  lifetime.abort();
  controller?.destroy();
  void invoke("fly_brain_exit").catch(() => {});
});
</script>

<style scoped>
.fly-brain-page {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: #a8d8f8;
}
</style>
