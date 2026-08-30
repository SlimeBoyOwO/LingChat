<template>
  <Transition name="full-access-notice">
    <div
      v-if="fullAccessUseVisible && !uiStore.showSettings"
      class="full-access-warning"
      role="status"
      aria-live="polite"
    >
      <ShieldAlert :size="18" aria-hidden="true" />
      <span>{{ $t('ui.toolCalls.fullAccessTopWarning') }}</span>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ShieldAlert } from 'lucide-vue-next'
import {
  currentToolAccessMode,
  currentToolActivity,
  getToolSettings,
} from '@/api/services/tool-settings'
import { useUIStore } from '@/stores/modules/ui/ui'

// 全权限模式下 LLM 执行敏感工具（文件写入/删除、命令执行等）时，
// 在顶部短暂亮出黄字警告；其余模式与普通只读调用不打扰。
const uiStore = useUIStore()

const fullAccessUseVisible = ref(false)
let fullAccessUseTimer: ReturnType<typeof setTimeout> | null = null
const fullAccessTools = new Set([
  'list_files',
  'read_file',
  'ReadMediaFile',
  'write_file',
  'delete_file',
  'edit_file',
  'search_files',
  'grep_files',
  'glob',
  'grep',
  'execute_command',
])

watch(currentToolActivity, (activity, previous) => {
  if (
    currentToolAccessMode.value !== 'full_access' ||
    activity?.status !== 'running' ||
    !fullAccessTools.has(activity.tool) ||
    (previous?.callId === activity.callId && previous.status === 'running')
  ) {
    return
  }
  fullAccessUseVisible.value = true
  if (fullAccessUseTimer) clearTimeout(fullAccessUseTimer)
  fullAccessUseTimer = setTimeout(() => {
    fullAccessUseVisible.value = false
    fullAccessUseTimer = null
  }, 2400)
})

onMounted(() => {
  void getToolSettings().catch((error) => {
    console.warn('[FullAccessWarning] 加载工具访问模式失败:', error)
  })
})

onBeforeUnmount(() => {
  if (fullAccessUseTimer) clearTimeout(fullAccessUseTimer)
})
</script>

<style scoped>
.full-access-warning {
  position: fixed;
  top: calc(15px + var(--safe-area-inset-top));
  left: 50%;
  z-index: 1001;
  display: flex;
  height: 40px;
  max-width: min(44rem, calc(100vw - 34rem));
  transform: translateX(-50%);
  align-items: center;
  gap: 0.45rem;
  padding: 0;
  color: rgb(250 204 21 / 78%);
  font-size: 0.875rem;
  font-weight: 700;
  letter-spacing: 0.02em;
  line-height: 1;
  pointer-events: none;
  white-space: nowrap;
  text-shadow:
    0 1px 2px rgb(0 0 0 / 95%),
    0 0 8px rgb(0 0 0 / 75%);
}

.full-access-notice-enter-active,
.full-access-notice-leave-active {
  transition: opacity 0.25s ease;
}

.full-access-notice-enter-from,
.full-access-notice-leave-to {
  opacity: 0;
}

@media (max-width: 1279px) {
  .full-access-warning {
    top: calc(54px + var(--safe-area-inset-top));
    max-width: calc(100vw - 2rem);
    width: max-content;
  }
}
</style>
