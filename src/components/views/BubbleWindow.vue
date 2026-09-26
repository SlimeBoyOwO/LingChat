<template>
  <!--
    气泡窗：只画「通知 + 气泡」，不承载交互（整窗点击穿透）。
    通知由 DialogueBox 自己钉在气泡顶边上方 —— 气泡高度随文本变化，通知因此
    始终紧贴气泡顶而不是窗口顶。显示内容全部来自宠物窗的镜像。
  -->
  <div
    id="bubble-app"
    :style="appStyleVars"
    class="relative overflow-hidden bg-transparent transition-none select-none"
  >
    <DialogueBox
      ref="dialogRef"
      :visible="bubbleVisible"
      :line="uiStore.showCharacterLine"
      :emotion="uiStore.showCharacterEmotion"
      :speed="uiStore.typeWriterSpeed"
      :instant="instant"
      :max-height="bubbleMaxHeight"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsStore } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useGameStore } from "@/stores/modules/game";
import DialogueBox from "../pet/DialogueBox.vue";
import { PET_BUBBLE_EVENT, PET_BUBBLE_REQUEST, type BubbleMirror } from "../pet/bubbleMirror";
import {
  BAND_BASE,
  DIALOG_MAX_BASE,
  NOTIFICATION_MAX_BASE,
  TAIL_OVERHANG_BASE,
} from "../pet/constants";

const settingsStore = useSettingsStore();
const uiStore = useUIStore();
const gameStore = useGameStore();

const dialogRef = ref<InstanceType<typeof DialogueBox> | null>(null);
void dialogRef;

/**
 * 是否整段显示、不播打字机。
 *
 * 只在**挂载后的首个非空台词**上为真：气泡窗可能比台词晚建（切进桌宠时），
 * 或因为改尺寸被重建，那些场景只该复现气泡现状，不该重播动画。
 * 之后任何一句都算新台词 —— 该播打字机。
 * 因此这里盯的是"台词是否变过"，而不是"挂载是否完成"。
 */
const instant = ref(true);
let firstLineSeen = false;

/**
 * 缩放**只认镜像值**：气泡窗的窗口尺寸是 Rust 按宠物窗的 scale 创建的，
 * 这里若读本窗口自己的 store，就可能在滑杆调整后与窗口实际尺寸错位。
 */
const scale = ref(settingsStore.pet?.scale || 1);
const px = (base: number) => `${Math.round(base * scale.value)}px`;

const bubbleVisible = computed(
  () => gameStore.currentStatus === "responding" && uiStore.showCharacterLine.trim() !== "",
);

/**
 * 气泡可用高度 = 整条气泡带。长尾在气泡盒内部、由容器抬升出的预留区承接，
 * 所以盒子取满带高也不会让尾巴越出窗口；超出这个高度的正文改为盒内滚动。
 */
const bubbleMaxHeight = computed(() => Math.round(BAND_BASE * scale.value));

const appStyleVars = computed(() => ({
  "--pet-ui-scale": scale.value.toString(),
  "--band-h": px(BAND_BASE),
  "--tail": px(TAIL_OVERHANG_BASE),
  "--dialog-h": px(DIALOG_MAX_BASE),
  "--notify-h": px(NOTIFICATION_MAX_BASE),
}));

/** 镜像投影：写进本窗口的 store，DialogueBox / PetNotification 即可直接渲染 */
const applyMirror = (m: BubbleMirror) => {
  if (m.petScale > 0) scale.value = m.petScale;
  // 首个非空台词整段显示（复现现状）；此后任何一句都是新台词 → 播打字机
  if (!firstLineSeen && m.line.trim()) {
    firstLineSeen = true;
    instant.value = true;
  } else if (firstLineSeen) {
    instant.value = false;
  }
  gameStore.currentStatus = m.status as typeof gameStore.currentStatus;
  uiStore.showCharacterLine = m.line;
  uiStore.showCharacterTitle = m.title;
  uiStore.showCharacterSubtitle = m.subtitle;
  uiStore.showCharacterEmotion = m.emotion;
  uiStore.showCharacterMotionText = m.motionText;
  settingsStore.setTextSpeed(m.textSpeed);
  uiStore.notification = m.notification as typeof uiStore.notification;
};

let unlisten: (() => void) | null = null;
const pingTimers: number[] = [];

onMounted(async () => {
  const appWindow = getCurrentWindow();
  document.body.style.backgroundColor = "transparent";
  document.documentElement.style.backgroundColor = "transparent";

  unlisten = await appWindow.listen<BubbleMirror>(PET_BUBBLE_EVENT, (event) =>
    applyMirror(event.payload),
  );

  // 握手重试：气泡窗创建早于宠物窗注册监听时，第一次请求会石沉大海，
  // 之后要等到下一句台词才会推送 —— 表现为「切进桌宠时当前台词丢失」。
  // 镜像本身幂等，重发几次的代价可以忽略。
  const ping = () => void appWindow.emit(PET_BUBBLE_REQUEST);
  ping();
  for (const ms of [120, 400, 1000]) {
    pingTimers.push(window.setTimeout(ping, ms));
  }
});

onUnmounted(() => {
  document.body.style.backgroundColor = "";
  document.documentElement.style.backgroundColor = "";
  unlisten?.();
  unlisten = null;
  pingTimers.forEach((t) => window.clearTimeout(t));
});
</script>

<style scoped>
#bubble-app {
  width: 100vw;
  height: 100dvh;
}
</style>
