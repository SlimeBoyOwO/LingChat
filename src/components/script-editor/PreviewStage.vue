<template>
  <Teleport to="body">
    <Transition name="preview">
      <div
        v-if="store.previewing"
        class="stage"
      >
        <!-- 复用真实的游戏渲染层。这是当初选「复用真引擎 + 真渲染层」而不是
             另写一套预览解释器的兑现点：这四个组件读的是同一份 store，
             引擎 emit 的事件经 eventQueue 进来后，表现与正式游玩逐帧一致。 -->
        <GameBackground />
        <GameRolesStage />
        <GameExtraUI />
        <GameDialog />

        <!-- 预览专属的顶栏，明确「这是试玩」而不是真在玩 -->
        <div class="bar">
          <span class="badge">试玩中</span>
          <span class="meta">{{ label }}</span>
          <span class="tip">调试不会记入通关，也不会解锁后续羁绊冒险</span>
          <button
            class="stop"
            @click="store.stopPreview()"
          >
            结束试玩
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import { GameBackground, GameDialog, GameRolesStage } from '@/components/game/standard'
import GameExtraUI from '@/components/game/standard/GameExtraUI.vue'
import { eventQueue } from '@/core/events/event-queue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

const store = useScriptEditorStore()

const props = defineProps<{ fromChapter?: string }>()

const label = computed(() => {
  const name = store.detail?.package.scriptName ?? ''
  return props.fromChapter ? `${name} · 从「${props.fromChapter}」开始` : name
})

/**
 * eventQueue 初始是 paused 的 —— 正式游玩里由 LoadingTransition 完成时 resume。
 * 编辑器没有那道转场，所以在预览打开时自己放行；关闭时 clear()，它会同时
 * 清空队列并把 paused 置回 true，免得残留事件泄漏到下一次试玩。
 */
watch(
  () => store.previewing,
  (on) => {
    if (on) {
      eventQueue.resume()
    } else {
      // clear() 内部会把 paused 置回 true，所以不需要另外 pause
      eventQueue.clear()
    }
  },
)
</script>

<style scoped>
.stage {
  position: fixed;
  inset: 0;
  z-index: 9990;
  overflow: hidden;
  background: #000;
}

.bar {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
  z-index: 10000;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  background: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent);
}
.badge {
  border: 1px solid rgba(121, 217, 255, 0.5);
  border-radius: 99px;
  padding: 2px 10px;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--accent-color);
  background: rgba(121, 217, 255, 0.15);
}
.meta {
  font-size: 0.78rem;
  color: #fff;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
}
.tip {
  font-size: 0.7rem;
  color: rgba(255, 255, 255, 0.6);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
}
.stop {
  margin-left: auto;
  border: 1px solid rgba(248, 113, 113, 0.45);
  border-radius: 0.5rem;
  padding: 5px 14px;
  font-size: 0.76rem;
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.16);
  backdrop-filter: blur(8px);
  transition: all 0.2s;
}
.stop:hover {
  color: #fff;
  background: rgba(248, 113, 113, 0.32);
}

.preview-enter-active,
.preview-leave-active {
  transition: opacity 0.25s cubic-bezier(0.18, 0.89, 0.32, 1);
}
.preview-enter-from,
.preview-leave-to {
  opacity: 0;
}
</style>
