import { ref, watch, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useGameStore } from "@/stores/modules/game";
import { eventQueue } from "@/core/events/event-queue";

/**
 * 前端上报的“当前是否适合投放主动对话”。
 * 条件：用户在聊天界面 且 设置面板未打开 且 输入框为空 且 这一轮台词已经播完。
 *
 * 仅在最终布尔值翻转时调用 invoke（不会反复上报）。
 */

// ===== 可投放的路由名（聊天主界面 + 桌宠） =====
const CHAT_ROUTES = ["LingChat", "PetMode"];

// ===== 全局输入状态（由 GameDialog / ChatInput 组件上报） =====
let _inputHasText = false;
const _inputListeners = new Set<() => void>();

/** 各输入组件在 watch 中调用此函数来更新输入状态 */
export function setInputHasText(val: boolean) {
  if (_inputHasText === val) return;
  _inputHasText = val;
  _inputListeners.forEach((fn) => fn());
}

export function useCanDeliver() {
  const router = useRouter();
  const uiStore = useUIStore();
  const gameStore = useGameStore();

  const canDeliver = ref(false);
  let lastInvoked: boolean | null = null;

  function recompute() {
    const onChatRoute = CHAT_ROUTES.includes(router.currentRoute.value.name as string);
    // 她还在说（生成中 / 台词没播完 / 等玩家点继续）时不算「可投放」：
    // 主动对话要等这一轮真的说完，否则新一轮会和当前台词叠在一起。
    // 光看状态不够：思考结束、状态被复位这些时机会把 currentStatus 提前打回 input，
    // 而队列里还排着没播的台词——必须队列也真的空了才算说完。
    const queue = eventQueue.getState();
    const dialogueIdle =
      gameStore.currentStatus === "input" && queue.queueLength === 0 && !queue.isProcessing;
    canDeliver.value = onChatRoute && !uiStore.showSettings && !_inputHasText && dialogueIdle;
  }

  // 监听变更
  watch(() => router.currentRoute.value.name, recompute, { immediate: true });
  watch(() => uiStore.showSettings, recompute);
  watch(() => gameStore.currentStatus, recompute);
  // 队列的变化不会改 currentStatus 的场合（状态提前复位）：队列空了也要重判一次
  watch(() => eventQueue.version.value, recompute);

  // 输入状态变化时重新计算
  _inputListeners.add(recompute);

  // 值翻转时通知后端
  watch(canDeliver, (val) => {
    if (val !== lastInvoked) {
      lastInvoked = val;
      invoke("proactive_set_can_deliver", { canDeliver: val }).catch((e) =>
        console.error("[CanDeliver] invoke failed:", e),
      );
    }
  });

  return { canDeliver };
}
