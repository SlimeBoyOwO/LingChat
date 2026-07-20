<template>
  <Teleport to="body">
    <Transition name="slide-up">
      <div
        v-if="visible"
        class="role-archive-toast"
        :data-phase="state.phase"
      >
        <div class="glow-effect"></div>

        <div class="archive-icon">
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

        <div class="archive-content">
          <div class="archive-header">
            <span class="archive-label">{{ label }}</span>
            <span v-if="state.phase === 'running' && state.percent >= 0" class="archive-percent">
              {{ state.percent }}%
            </span>
          </div>
          <div class="archive-title">{{ title }}</div>
          <div v-if="message" class="archive-description">{{ message }}</div>

          <div v-if="state.phase === 'running'" class="bar-track">
            <div class="bar-fill" :style="barStyle"></div>
          </div>

          <div v-if="state.phase === 'error'" class="archive-actions">
            <button class="action-btn" @click="dismiss">关闭</button>
          </div>
        </div>

        <button v-if="state.phase === 'running'" class="cancel-btn" @click="onCancel">
          取消
        </button>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, watch, onUnmounted } from 'vue'
import { useRoleImportExport } from '@/composables/useRoleImportExport'

const { store, cancel } = useRoleImportExport()

const activeKey = computed<'import' | 'export'>(() =>
  store.import.phase !== 'idle' ? 'import' : 'export',
)

const state = computed(() => (activeKey.value === 'import' ? store.import : store.export))

const visible = computed(() => state.value.phase !== 'idle')

const label = computed(() => {
  const p = state.value.phase
  const kind = activeKey.value === 'import' ? '导入' : '导出'
  if (p === 'running') return `${kind}中`
  if (p === 'done') return `${kind}成功`
  if (p === 'error') return `${kind}失败`
  if (p === 'cancelled') return `已取消`
  return kind
})

const title = computed(() => {
  if (activeKey.value === 'import') {
    return store.import.fileName || '角色压缩包'
  }
  return store.export.roleName || '角色导出'
})

const message = computed(() => {
  const s = state.value
  if (s.phase === 'error') return s.error || s.message
  return s.message
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

<style scoped>
@reference "tailwindcss";

.role-archive-toast {
  @apply fixed bottom-8 right-8 z-[9999];
  @apply flex items-center gap-4;
  @apply p-4 min-w-[340px] max-w-[440px];
  @apply overflow-hidden;
  @apply rounded-xl;

  background: rgba(15, 15, 15, 0.55);
  backdrop-filter: blur(20px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

.role-archive-toast[data-phase="running"] {
  border: 1px solid rgba(121, 217, 255, 0.25);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.6),
    0 0 15px rgba(121, 217, 255, 0.12) inset;
}
.role-archive-toast[data-phase="running"] .glow-effect {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 150%;
  height: 150%;
  background: radial-gradient(circle, rgba(121, 217, 255, 0.1) 0%, transparent 60%);
  z-index: -1;
  filter: blur(20px);
}
.role-archive-toast[data-phase="running"] .archive-label { @apply text-cyan-300; }
.role-archive-toast[data-phase="running"] .archive-icon { @apply text-cyan-300; @apply bg-cyan-300/10; }
.role-archive-toast[data-phase="running"] .bar-fill { @apply bg-cyan-300; }

.role-archive-toast[data-phase="done"] {
  border: 1px solid rgba(74, 222, 128, 0.25);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.6),
    0 0 15px rgba(74, 222, 128, 0.12) inset;
}
.role-archive-toast[data-phase="done"] .glow-effect {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 150%;
  height: 150%;
  background: radial-gradient(circle, rgba(74, 222, 128, 0.12) 0%, transparent 60%);
  z-index: -1;
  filter: blur(20px);
}
.role-archive-toast[data-phase="done"] .archive-label { @apply text-green-400; }
.role-archive-toast[data-phase="done"] .archive-icon { @apply text-green-400; @apply bg-green-400/10; }
.role-archive-toast[data-phase="done"] .bar-fill { @apply bg-green-400; }

.role-archive-toast[data-phase="error"] {
  border: 1px solid rgba(248, 113, 113, 0.3);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.6),
    0 0 15px rgba(248, 113, 113, 0.15) inset;
}
.role-archive-toast[data-phase="error"] .glow-effect {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 150%;
  height: 150%;
  background: radial-gradient(circle, rgba(248, 113, 113, 0.12) 0%, transparent 60%);
  z-index: -1;
  filter: blur(20px);
}
.role-archive-toast[data-phase="error"] .archive-label { @apply text-red-400; }
.role-archive-toast[data-phase="error"] .archive-icon { @apply text-red-400; @apply bg-red-400/[0.12]; }

.role-archive-toast[data-phase="cancelled"] {
  border: 1px solid rgba(156, 163, 175, 0.25);
}
.role-archive-toast[data-phase="cancelled"] .glow-effect { display: none; }
.role-archive-toast[data-phase="cancelled"] .archive-label { @apply text-gray-400; }
.role-archive-toast[data-phase="cancelled"] .archive-icon { @apply text-gray-400; @apply bg-gray-400/10; }

.archive-icon {
  @apply shrink-0 w-12 h-12 rounded-lg flex items-center justify-center;
}
.archive-content {
  @apply flex flex-col justify-center gap-0.5 flex-1 min-w-0;
}
.archive-header {
  @apply flex items-center justify-between gap-2;
}
.archive-label {
  @apply text-xs font-bold tracking-wider;
}
.archive-percent {
  @apply text-xs font-bold text-white/70;
}
.archive-title {
  @apply text-white font-bold text-sm leading-tight truncate;
}
.archive-description {
  @apply text-gray-300 text-xs leading-tight break-all;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.bar-track {
  @apply mt-2 w-full h-1 rounded-full bg-white/10 overflow-hidden;
}
.bar-fill {
  @apply h-full rounded-full;
  width: 0%;
}

.archive-actions {
  @apply mt-2 flex gap-2;
}
.action-btn {
  @apply px-3 py-1 rounded-md text-xs font-medium cursor-pointer;
  @apply bg-white/10 text-white hover:bg-white/20 transition-colors;
}
.cancel-btn {
  @apply shrink-0 self-start px-3 py-1.5 rounded-md text-xs font-medium cursor-pointer;
  @apply bg-white/10 text-white/80 hover:bg-red-500/30 hover:text-white transition-colors;
}

@keyframes archive-shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(80px) scale(0.9);
  opacity: 0;
}
</style>
