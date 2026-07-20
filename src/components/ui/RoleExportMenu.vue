<template>
  <div class="relative">
    <button
      class="export-trigger"
      :title="'导出角色'"
      @click.stop="toggle"
    >
      <Upload :size="24" />
    </button>

    <Transition name="menu-pop">
      <div v-if="open" class="export-menu" @click.stop>
        <div class="menu-header">{{ roleName ? `${roleName} - 导出` : '导出角色' }}</div>
        <button class="menu-item" :disabled="busy" @click="choose('zip')">
          <span class="dot dot-zip"></span>
          <span class="label">ZIP</span>
          <span class="hint">{{ busy && format === 'zip' ? '处理中...' : '通用压缩' }}</span>
        </button>
        <button class="menu-item" :disabled="busy" @click="choose('7z')">
          <span class="dot dot-7z"></span>
          <span class="label">7Z</span>
          <span class="hint">{{ busy && format === '7z' ? '处理中...' : '高压缩率' }}</span>
        </button>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { Upload } from 'lucide-vue-next'
import { useRoleImportExport } from '@/composables/useRoleImportExport'
import type { ArchiveFormat } from '@/api/services/role-archive'

const props = defineProps<{
  roleId: number
  roleName: string
}>()

const { store, doExport } = useRoleImportExport()
const open = ref(false)
const format = ref<ArchiveFormat>('zip')

const busy = ref(false)

function toggle() {
  open.value = !open.value
}

function close() {
  open.value = false
}

async function choose(fmt: ArchiveFormat) {
  if (busy.value) return
  format.value = fmt
  busy.value = true
  close()
  try {
    await doExport(props.roleId, props.roleName, fmt)
  } finally {
    busy.value = false
  }
}

function onDocClick() {
  close()
}

onMounted(() => {
  // 延迟注册，避免捕获刚刚用于打开菜单的点击事件。
  setTimeout(() => document.addEventListener('click', onDocClick), 0)
})
onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
})
</script>

<style scoped>
@reference "tailwindcss";

.export-trigger {
  @apply flex items-center justify-center p-1 rounded-full;
  @apply bg-black/5 text-white/60 hover:text-white hover:bg-white/10;
  @apply transition-all;
}

.export-menu {
  @apply absolute top-full right-0 z-50 min-w-[180px];
  @apply rounded-xl overflow-hidden;
  @apply p-1.5;
  background: rgba(15, 15, 15, 0.85);
  backdrop-filter: blur(20px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.menu-header {
  @apply px-3 py-1.5 text-xs font-bold text-white/50 tracking-wider;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  margin-bottom: 4px;
}

.menu-item {
  @apply w-full flex items-center gap-2.5 px-3 py-2 rounded-lg;
  @apply text-left cursor-pointer transition-colors;
  @apply text-white/80 hover:bg-white/10 hover:text-white;
}
.menu-item:disabled {
  @apply opacity-50 cursor-not-allowed;
}

.dot {
  @apply w-2 h-2 rounded-full shrink-0;
}
.dot-zip {
  background: #fbbf24;
}
.dot-7z {
  background: #79d9ff;
}

.label {
  @apply text-sm font-bold w-8;
}
.hint {
  @apply text-xs text-white/40;
}

.menu-pop-enter-active,
.menu-pop-leave-active {
  transition: all 0.18s ease;
}
.menu-pop-enter-from,
.menu-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.96);
}
</style>
