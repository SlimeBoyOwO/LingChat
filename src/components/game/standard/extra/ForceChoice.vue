<template>
  <!-- 强制选择（DDLC 式）：真实鼠标被磁力强行拖向指定选项，但点击必须玩家自己动手 -->
  <div
    v-if="gameStore.forceChoice"
    ref="overlayRef"
    class="force-choice-overlay"
    @mousemove="onMouseMove"
  >
    <div ref="choicesRef" class="flex w-full max-w-2xl flex-col gap-10 px-4">
      <button
        v-for="choice in gameStore.forceChoice.choices"
        :key="choice.text"
        :disabled="submitting || choice.disabled || choice.text !== gameStore.forceChoice!.forced"
        :title="choice.disabled ? choice.reason || '该选项当前不可选' : ''"
        :class="[
          'relative w-full rounded-full border border-white/10 px-8 py-4 backdrop-blur-xl backdrop-saturate-150',
          choice.disabled && choice.text !== gameStore.forceChoice!.forced
            ? 'bg-slate-900/20 text-white/30'
            : choice.text === gameStore.forceChoice!.forced
              ? 'forced-target bg-slate-900/40 text-white'
              : 'bg-slate-900/20 text-white/40',
        ]"
        @click="onChoiceClick(choice)"
      >
        <span
          class="block text-center text-lg font-medium tracking-widest
            drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]"
        >
          {{ choice.text }}
        </span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import { useGameStore } from "@/stores/modules/game";
  import type { ScriptChoiceItem } from "@/types/script";

  const gameStore = useGameStore();

  const overlayRef = ref<HTMLElement | null>(null);
  const choicesRef = ref<HTMLElement | null>(null);

  // 当前真实鼠标位置（由 mousemove 追踪；拖动期间会被我们不断改写）
  const realPos = { x: window.innerWidth / 2, y: window.innerHeight / 2 };
  type ScriptCursorPosition = { x: number; y: number };

  let timerId = 0;
  let warpStopTimerId = 0;
  let runGeneration = 0;
  let activeRequestId = "";
  let warpActive = false;
  let submitted = false;
  let submitting = false;
  let warpFailures = 0;

  /**
   * 按"选项在 choices 里的索引"从容器里取强制目标按钮。
   * 不用模板 :ref 条件绑定——函数 ref 在 v-for 重渲染下的回调时序不可靠，
   * 曾经出现 ref 指到错误按钮、鼠标被拖向相反选项的问题。
   */
  function forcedBtn(): HTMLElement | null {
    const fc = gameStore.forceChoice;
    const root = overlayRef.value;
    if (!fc || !root) return null;
    const idx = fc.choices.findIndex((c) => c.text === fc.forced);
    if (idx < 0) return null;
    const buttons = root.querySelectorAll("button");
    return (buttons.item(idx) as HTMLElement) ?? null;
  }

  function onMouseMove(e: MouseEvent) {
    // 玩家挣扎时取最新位置作为下一次拖拽的起点
    realPos.x = e.clientX;
    realPos.y = e.clientY;
  }

  const TICK_MS = 1000 / 30; // DDLC RigMouse 原版：每秒 30 次
  const PULL_RATIO = 0.1; // (current * 9 + target) / 10
  const MAX_WARP_FAILURES = 5; // 连续失败这么多次就放弃拖动，留在原地等玩家自己点

  async function tick(generation: number) {
    const fc = gameStore.forceChoice;
    if (
      generation !== runGeneration ||
      !fc ||
      fc.requestId !== activeRequestId ||
      !warpActive ||
      submitted
    )
      return;
    const btn = forcedBtn();
    if (!btn) {
      // 按钮尚未渲染完成：重试而不是静默退出，避免拖动完全不发生
      timerId = window.setTimeout(() => tick(generation), TICK_MS);
      return;
    }

    const rect = btn.getBoundingClientRect();
    const tx = rect.left + rect.width / 2;
    const ty = rect.top + rect.height / 2;

    // DDLC 原版 RigMouse：每拍把真实指针与目标按 9:1 混合，玩家越挣扎越能感到
    // 一股持续拉力，而不是第一帧突然瞬移到按钮上。
    realPos.x = realPos.x * (1 - PULL_RATIO) + tx * PULL_RATIO;
    realPos.y = realPos.y * (1 - PULL_RATIO) + ty * PULL_RATIO;

    // Only constrain each scripted warp to the visible choice panel. No OS-wide
    // cursor clipping: Esc, blur and the existing five-second deadline release it.
    const panel = choicesRef.value?.getBoundingClientRect();
    if (panel) {
      const left = Math.max(0, panel.left);
      const right = Math.min(window.innerWidth - 1, panel.right);
      const top = Math.max(0, panel.top);
      const bottom = Math.min(window.innerHeight - 1, panel.bottom);
      if (right <= left || bottom <= top) {
        stopWarp("panel-not-visible");
        return;
      }
      realPos.x = Math.min(right, Math.max(left, realPos.x));
      realPos.y = Math.min(bottom, Math.max(top, realPos.y));
    }

    try {
      await invoke("warp_cursor", {
        requestId: fc.requestId,
        x: realPos.x,
        y: realPos.y,
      });
      warpFailures = 0;
    } catch (e) {
      warpFailures += 1;
      console.warn(`[ForceChoice] warp_cursor 失败(${warpFailures}/${MAX_WARP_FAILURES}):`, e);
      if (warpFailures >= MAX_WARP_FAILURES) {
        // 拖不动就放弃拖动、保持选项开着等玩家自己点——只有强制项可点，不会死锁
        console.warn("[ForceChoice] warp_cursor 持续失败，停止牵引并保留 forced 手动点击");
        stopWarp("warp-failed");
        return;
      }
      timerId = window.setTimeout(() => tick(generation), TICK_MS);
      return;
    }

    if (generation !== runGeneration || !warpActive || submitted) return;
    timerId = window.setTimeout(() => tick(generation), TICK_MS);
  }

  /**
   * 只停止系统鼠标牵引，不替玩家点击，也不把指针恢复到旧位置。
   * Esc、失焦、隐藏、5 秒时限、事件切换和卸载都走这一条幂等清理路径。
   */
  function stopWarp(reason: string) {
    if (!warpActive && !activeRequestId) return;
    const requestId = activeRequestId;
    activeRequestId = "";
    warpActive = false;
    runGeneration += 1;
    clearTimeout(timerId);
    clearTimeout(warpStopTimerId);
    if (requestId) {
      invoke("cancel_script_cursor_warp", { requestId }).catch((error) => {
        console.warn(`[ForceChoice] 取消鼠标牵引失败(${reason}):`, error);
      });
    }
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") stopWarp("escape");
  }

  function onWindowBlur() {
    stopWarp("blur");
  }

  function onVisibilityChange() {
    if (document.visibilityState !== "visible") stopWarp("hidden");
  }

  /** 玩家自己点击：只有未被禁用的强制项会真正提交（其余按钮本就 disabled） */
  async function onChoiceClick(choice: ScriptChoiceItem) {
    const fc = gameStore.forceChoice;
    if (!fc || submitted || submitting) return;
    if (choice.disabled || choice.text !== fc.forced) return;
    submitting = true;
    stopWarp("submit");
    try {
      await invoke("script_submit_choice", {
        choice: choice.text,
        requestId: fc.requestId,
      });
      submitted = true;
      gameStore.appendGameMessage({
        type: "message",
        displayName: gameStore.userName,
        content: choice.text,
      });
      if (gameStore.forceChoice?.requestId === fc.requestId) gameStore.forceChoice = null;
    } catch (error) {
      console.error("[ForceChoice] 提交 forced 选项失败:", error);
    } finally {
      submitting = false;
    }
  }

  watch(
    () => gameStore.forceChoice,
    async (fc) => {
      stopWarp("event-change");
      activeRequestId = fc?.requestId ?? "";
      submitted = false;
      submitting = false;
      warpFailures = 0;
      if (!fc) return;
      if (!fc.forced || !fc.choices.some((c) => c.text === fc.forced && !c.disabled)) {
        console.error("[ForceChoice] forced 配置无效，拒绝自动提交");
        return;
      }

      const generation = ++runGeneration;
      warpActive = true;
      await nextTick();
      try {
        const position = await invoke<ScriptCursorPosition>("get_script_cursor_position", {
          requestId: fc.requestId,
        });
        realPos.x = position.x;
        realPos.y = position.y;
      } catch (error) {
        console.warn("[ForceChoice] 无法读取真实鼠标位置，退化为 forced 手动点击:", error);
        stopWarp("position-unavailable");
        return;
      }
      if (generation !== runGeneration || !warpActive || submitted) return;
      warpStopTimerId = window.setTimeout(() => stopWarp("time-limit"), 5000);
      timerId = window.setTimeout(() => tick(generation), TICK_MS);
    }
  );

  onMounted(() => {
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("blur", onWindowBlur);
    document.addEventListener("visibilitychange", onVisibilityChange);
  });

  onBeforeUnmount(() => {
    stopWarp("unmount");
    window.removeEventListener("keydown", onKeyDown, true);
    window.removeEventListener("blur", onWindowBlur);
    document.removeEventListener("visibilitychange", onVisibilityChange);
  });
</script>

<style scoped>
  .force-choice-overlay {
    position: fixed;
    inset: 0;
    z-index: 900000;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    margin-top: -15vh;
    /* 父容器 GameExtraUI 是 pointer-events:none，必须显式夺回事件 */
    pointer-events: auto;
  }

  /* 非强制项在演出期间不可点 */
  .force-choice-overlay button:disabled {
    cursor: not-allowed;
  }

  /* 被吸附的目标按钮：血色呼吸微光，像有什么在"推荐"它 */
  .forced-target {
    animation: forced-breathe 1.4s ease-in-out infinite;
  }

  @keyframes forced-breathe {
    0%,
    100% {
      box-shadow: 0 0 8px rgba(184, 9, 26, 0.25);
      border-color: rgba(184, 9, 26, 0.35);
    }
    50% {
      box-shadow: 0 0 22px rgba(184, 9, 26, 0.55);
      border-color: rgba(184, 9, 26, 0.8);
    }
  }
</style>
