<template>
  <Teleport to="body">
    <Transition name="slide-up">
      <div
        v-if="visible"
        :data-phase="state.phase"
        class="bar fixed bottom-[calc(32px+var(--safe-area-inset-bottom))] right-8 z-[9999] flex items-center gap-4 p-4 min-w-[340px] max-w-[440px] overflow-hidden rounded-xl backdrop-blur-[20px]"
      >
        <div
          v-if="state.phase !== 'cancelled'"
          class="glow absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[150%] h-[150%] -z-10 blur-[20px]"
          :style="glowStyle"
        ></div>

        <div
          class="icon-wrap shrink-0 w-12 h-12 rounded-lg flex items-center justify-center"
        >
          <svg
            v-if="state.phase === 'running'"
            xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            class="w-7 h-7 animate-spin"
          >
            <path d="M21 12a9 9 0 1 1-6.219-8.56" />
          </svg>
          <svg
            v-else-if="state.phase === 'done'"
            xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            class="w-7 h-7"
          >
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
            <polyline points="22 4 12 14.01 9 11.01" />
          </svg>
          <svg
            v-else-if="state.phase === 'error'"
            xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            class="w-7 h-7"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <svg
            v-else
            xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            class="w-7 h-7"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="15" y1="9" x2="9" y2="15" />
            <line x1="9" y1="9" x2="15" y2="15" />
          </svg>
        </div>

        <div class="flex flex-col justify-center gap-0.5 flex-1 min-w-0">
          <div class="flex items-center justify-between gap-2">
            <span class="label-text text-xs font-bold tracking-wider">{{ label }}</span>
            <span
              v-if="state.phase === 'running' && state.percent >= 0"
              class="text-xs font-bold text-white/70"
            >{{ state.percent }}%</span>
          </div>
          <div class="text-white font-bold text-sm leading-tight truncate">{{ title }}</div>
          <div
            v-if="message"
            class="text-gray-300 text-xs leading-tight break-all line-clamp-2"
          >{{ message }}</div>

          <div
            v-if="state.phase === 'running'"
            class="mt-2 w-full h-1 rounded-full bg-white/10 overflow-hidden"
          >
            <div
              class="fill h-full rounded-full"
              :style="barStyle"
            ></div>
          </div>

          <div
            v-if="state.phase === 'error'"
            class="mt-2 flex gap-2"
          >
            <button
              class="btn-close px-3 py-1 rounded-md text-xs font-medium cursor-pointer text-white"
              @click="dismiss"
            >{{ $t('ui.archiveProgress.close') }}</button>
          </div>
        </div>

        <button
          v-if="state.phase === 'running'"
          class="btn-cancel shrink-0 self-start px-3 py-1.5 rounded-md text-xs font-medium cursor-pointer text-white/80"
          @click="onCancel"
        >{{ $t('ui.archiveProgress.cancel') }}</button>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, watch, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoleImportExport } from '@/composables/useRoleImportExport'

const { t } = useI18n()
const { store, cancel } = useRoleImportExport()

type Phase = 'idle' | 'running' | 'done' | 'error' | 'cancelled'

const activeKey = computed<'import' | 'export'>(() =>
  store.import.phase !== 'idle' ? 'import' : 'export',
)

const state = computed(() => (activeKey.value === 'import' ? store.import : store.export))

const visible = computed(() => state.value.phase !== 'idle')

const label = computed(() => {
  const p = state.value.phase
  const isImport = activeKey.value === 'import'
  if (p === 'running') return isImport ? t('ui.archiveProgress.importing') : t('ui.archiveProgress.exporting')
  if (p === 'done') return isImport ? t('ui.archiveProgress.importSuccess') : t('ui.archiveProgress.exportSuccess')
  if (p === 'error') return isImport ? t('ui.archiveProgress.importFailed') : t('ui.archiveProgress.exportFailed')
  if (p === 'cancelled') return t('ui.archiveProgress.cancelled')
  return isImport ? t('ui.archiveProgress.kindImport') : t('ui.archiveProgress.kindExport')
})

const title = computed(() => {
  if (activeKey.value === 'import') {
    return store.import.fileName || t('ui.archiveProgress.defaultImportTitle')
  }
  return store.export.roleName || t('ui.archiveProgress.defaultExportTitle')
})

const message = computed(() => {
  const s = state.value
  if (s.phase === 'error') return s.error || s.message
  return s.message
})

// 阶段配色：4 个状态切换主容器的描边、阴影与子元素颜色。
// running 变体统一用 #79d9ff（rgb 121,217,255），与原 border/box-shadow 颜色一致，
// 之前散落的 cyan-300 (#67e8f9) 在本次 inline 化时统一为同一个色源。
const GLOW_COLORS: Record<Phase, string> = {
  idle: '',
  running: 'rgba(121, 217, 255, 0.1)',
  done: 'rgba(74, 222, 128, 0.12)',
  error: 'rgba(248, 113, 113, 0.12)',
  cancelled: '',
}

const glowStyle = computed(() => {
  const color = GLOW_COLORS[state.value.phase]
  if (!color) return {}
  return {
    background: `radial-gradient(circle, ${color} 0%, transparent 60%)`,
  }
})

const barStyle = computed(() => {
  const pct = state.value.percent
  if (pct < 0) {
    return { width: '100%', animation: 'archive-shimmer 1.2s ease-in-out infinite' }
  }
  return { width: `${pct}%`, transition: 'width 0.3s ease' }
})

let dismissTimer: number | null = null
function clearDismiss() {
  if (dismissTimer !== null) {
    window.clearTimeout(dismissTimer)
    dismissTimer = null
  }
}
function scheduleDismiss(ms: number) {
  clearDismiss()
  dismissTimer = window.setTimeout(() => {
    if (activeKey.value === 'import') store.resetImport()
    else store.resetExport()
  }, ms)
}

watch(
  () => state.value.phase,
  (phase) => {
    if (phase === 'done') scheduleDismiss(3000)
    else if (phase === 'cancelled') scheduleDismiss(2500)
    else if (phase === 'error') scheduleDismiss(10000)
    else clearDismiss()
  },
)

function onCancel() {
  cancel()
}
function dismiss() {
  clearDismiss()
  if (activeKey.value === 'import') store.resetImport()
  else store.resetExport()
}

onUnmounted(() => clearDismiss())
</script>

<style>
/* Vue scoped 会给 @keyframes 追加 hash 后缀，把 shimmer 改成 archive-shimmer-xxxxx。
   ImportProgressBar.vue:163 的 inline style 引用的是原始名 archive-shimmer，
   因此 shimmer 动画从不会触发。这里把 keyframe 放到非 scoped 块，
   名字保持不变，让 inline animation 能匹配上。*/
@keyframes archive-shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}
</style>

<style scoped>
/* 仅保留 Tailwind 无法表达的两条动画：进度条 indeterminate shimmer + Toast 进出 */

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(80px) scale(0.9);
  opacity: 0;
}

/* 阶段配色：4 个状态切换主容器的描边、阴影与子元素颜色。
   running 变体统一用 #79d9ff（rgb 121,217,255），与原 border/box-shadow 颜色一致，
   之前散落的 cyan-300 (#67e8f9) 在本次 inline 化时统一为同一个色源。
   全部使用静态选择器（[data-phase="..."] + 语义 class），避免 Tailwind JIT 
   扫描不到动态拼接的 class。*/

.bar {
  background: rgba(15, 15, 15, 0.55);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

[data-phase="running"].bar {
  border-color: rgba(121, 217, 255, 0.25);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6), inset 0 0 15px rgba(121, 217, 255, 0.12);
}
[data-phase="running"] .icon-wrap {
  color: #79d9ff;
  background: rgba(121, 217, 255, 0.1);
}
[data-phase="running"] .label-text { color: #79d9ff; }
[data-phase="running"] .fill { background: #79d9ff; }

[data-phase="done"].bar {
  border-color: rgba(74, 222, 128, 0.25);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6), inset 0 0 15px rgba(74, 222, 128, 0.12);
}
[data-phase="done"] .icon-wrap {
  color: #4ade80;
  background: rgba(74, 222, 128, 0.1);
}
[data-phase="done"] .label-text { color: #4ade80; }
[data-phase="done"] .fill { background: #4ade80; }

[data-phase="error"].bar {
  border-color: rgba(248, 113, 113, 0.3);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6), inset 0 0 15px rgba(248, 113, 113, 0.15);
}
[data-phase="error"] .icon-wrap {
  color: #f87171;
  background: rgba(248, 113, 113, 0.12);
}
[data-phase="error"] .label-text { color: #f87171; }

[data-phase="cancelled"].bar {
  border-color: rgba(156, 163, 175, 0.25);
}
[data-phase="cancelled"] .icon-wrap {
  color: #9ca3af;
  background: rgba(156, 163, 175, 0.1);
}
[data-phase="cancelled"] .label-text { color: #9ca3af; }

.btn-close {
  background: rgba(255, 255, 255, 0.1);
  transition: background-color 0.2s;
}
.btn-close:hover {
  background: rgba(255, 255, 255, 0.2);
}

.btn-cancel {
  background: rgba(255, 255, 255, 0.1);
  transition: background-color 0.2s, color 0.2s;
}
.btn-cancel:hover {
  background: rgba(239, 68, 68, 0.3);
  color: #ffffff;
}
</style>