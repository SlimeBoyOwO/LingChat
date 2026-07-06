<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="visible && phase !== 'idle'"
        class="fixed inset-0 z-[10000] flex items-center justify-center bg-slate-900/60 backdrop-blur-sm"
      >
        <div class="w-[520px] max-h-[80vh] rounded-3xl bg-slate-800/90 shadow-2xl flex flex-col overflow-hidden">
          <!-- 头部 -->
          <div class="px-6 pt-6 pb-4">
            <h2 class="text-xl font-semibold text-white">{{ dialogTitle }}</h2>
            <p v-if="subtitle" class="text-sm text-slate-400 mt-1">{{ subtitle }}</p>
          </div>

          <!-- 内容区 -->
          <div class="px-6 pb-2 flex-1 overflow-y-auto scrollbar-hide">
            <!-- 评审阶段：文件差异列表 -->
            <template v-if="phase === 'review' && syncInfo">
              <!-- 统计栏 -->
              <div class="flex items-center justify-between mb-3 text-sm text-slate-300 bg-slate-700/50 rounded-xl px-4 py-2">
                <span>共 {{ totalCount }} 个变更</span>
                <span>已选 {{ selectedCount }} 个</span>
                <span>{{ formatBytes(syncInfo.totalSize) }}</span>
                <div class="flex gap-2">
                  <button class="text-xs text-blue-400 hover:text-blue-300" @click="selectAll">全选</button>
                  <button class="text-xs text-slate-400 hover:text-slate-300" @click="deselectAll">全不选</button>
                </div>
              </div>

              <!-- 文件树：按目录分组 -->
              <div class="max-h-[50vh] overflow-y-auto space-y-1">
                <template v-for="group in fileGroups" :key="group.dir">
                  <!-- 目录头 -->
                  <div
                    class="flex items-center gap-2 px-2 py-1 cursor-pointer hover:bg-slate-700/40 rounded"
                    @click="toggleGroup(group.dir)"
                  >
                    <Minus v-if="group.expanded" :size="14" class="text-slate-500" />
                    <Plus v-else :size="14" class="text-slate-500" />
                    <Folder :size="16" class="text-sky-400" />
                    <span class="text-sm text-slate-300 font-medium">{{ group.dir }}</span>
                    <span class="text-xs text-slate-500">({{ group.files.length }})</span>
                  </div>

                  <!-- 文件列表 -->
                  <template v-if="group.expanded">
                    <div
                      v-for="file in group.files"
                      :key="file.path"
                      class="flex items-center gap-2 pl-8 pr-2 py-1 hover:bg-slate-700/30 rounded"
                    >
                      <input
                        type="checkbox"
                        :checked="selectedFiles.has(file.path)"
                        @change="toggleFile(file.path)"
                        class="w-4 h-4 accent-blue-500 rounded"
                      />
                      <span class="flex-1 text-sm text-slate-300 truncate" :title="file.path">
                        {{ file.path.replace(group.dir + '/', '') }}
                      </span>
                      <span
                        :class="[
                          'text-xs px-1.5 py-0.5 rounded-full',
                          file.changeType === 'add' ? 'bg-emerald-500/20 text-emerald-400' : 'bg-amber-500/20 text-amber-400',
                        ]"
                      >
                        {{ file.changeType === 'add' ? '新增' : '修改' }}
                      </span>
                      <span class="text-xs text-slate-500 w-16 text-right">{{ formatBytes(file.size) }}</span>
                    </div>
                  </template>
                </template>
              </div>
            </template>

            <!-- 同步中 -->
            <template v-else-if="phase === 'syncing'">
              <div class="flex flex-col items-center py-8 gap-4">
                <RefreshCw :size="40" class="text-blue-400 animate-spin" />
                <p class="text-slate-300">正在同步文件...</p>
              </div>
            </template>

            <!-- 完成 -->
            <template v-else-if="phase === 'complete'">
              <div class="flex flex-col items-center py-8 gap-4">
                <CheckCircle :size="48" class="text-emerald-400" />
                <p class="text-slate-300">数据资源同步完成</p>
              </div>
            </template>

            <!-- 错误 -->
            <template v-else-if="phase === 'error'">
              <div class="flex flex-col items-center py-8 gap-4">
                <XCircle :size="48" class="text-red-400" />
                <p class="text-red-400 text-sm text-center">{{ errorMessage || '同步失败' }}</p>
              </div>
            </template>
          </div>

          <!-- 底部按钮 -->
          <div class="px-6 py-4 flex justify-end gap-3 border-t border-slate-700/50">
            <template v-if="phase === 'review'">
              <button class="px-4 py-2 text-sm text-slate-400 hover:text-slate-200" @click="$emit('close')">
                取消
              </button>
              <button
                class="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-500 disabled:opacity-50"
                :disabled="selectedCount === 0"
                @click="handleApply"
              >
                同步选中文件
              </button>
            </template>
            <template v-else-if="phase === 'complete' || phase === 'error'">
              <button class="px-4 py-2 text-sm bg-slate-600 text-white rounded-lg hover:bg-slate-500" @click="$emit('close')">
                好的
              </button>
            </template>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import {
  Folder,
  Minus,
  Plus,
  RefreshCw,
  CheckCircle,
  XCircle,
} from 'lucide-vue-next'
import type { ResourceSyncInfo, SyncFileEntry } from '@/composables/useUpdater'

// ─── Props & Emits ───────────────────────────────────────────

const props = defineProps<{
  visible: boolean
  phase: 'idle' | 'review' | 'syncing' | 'complete' | 'error'
  syncInfo: ResourceSyncInfo | null
  errorMessage: string
}>()

const emit = defineEmits<{
  close: []
  apply: [selectedFiles: string[]]
}>()

// ─── 选中状态 ────────────────────────────────────────────────

const selectedFiles = ref<Set<string>>(new Set())

// 重置选中状态
watch(
  () => props.syncInfo,
  () => {
    selectedFiles.value = new Set()
  },
)

const selectedCount = computed(() => selectedFiles.value.size)
const totalCount = computed(() => {
  if (!props.syncInfo) return 0
  return props.syncInfo.filesToAdd.length + props.syncInfo.filesToModify.length
})

function toggleFile(path: string) {
  const next = new Set(selectedFiles.value)
  if (next.has(path)) {
    next.delete(path)
  } else {
    next.add(path)
  }
  selectedFiles.value = next
}

function selectAll() {
  if (!props.syncInfo) return
  const all = [
    ...props.syncInfo.filesToAdd.map((f) => f.path),
    ...props.syncInfo.filesToModify.map((f) => f.path),
  ]
  selectedFiles.value = new Set(all)
}

function deselectAll() {
  selectedFiles.value = new Set()
}

// ─── 文件分组 ────────────────────────────────────────────────

interface FileGroup {
  dir: string
  files: SyncFileEntry[]
  expanded: boolean
}

const expandedGroups = ref<Set<string>>(new Set())

const fileGroups = computed<FileGroup[]>(() => {
  if (!props.syncInfo) return []

  const allFiles = [
    ...props.syncInfo.filesToAdd,
    ...props.syncInfo.filesToModify,
  ]

  const groups = new Map<string, SyncFileEntry[]>()
  for (const file of allFiles) {
    // 取第一级子目录或根
    const parts = file.path.split('/')
    const dir = parts.length > 1 ? parts.slice(0, -1).join('/') : '(根)'
    if (!groups.has(dir)) groups.set(dir, [])
    groups.get(dir)!.push(file)
  }

  return [...groups.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([dir, files]) => ({
      dir,
      files: files.sort((a, b) => a.path.localeCompare(b.path)),
      expanded: expandedGroups.value.has(dir),
    }))
})

function toggleGroup(dir: string) {
  const next = new Set(expandedGroups.value)
  if (next.has(dir)) {
    next.delete(dir)
  } else {
    next.add(dir)
  }
  expandedGroups.value = next
}

// 默认展开所有组
watch(
  () => props.phase,
  (p) => {
    if (p === 'review' && props.syncInfo) {
      const dirs = new Set<string>()
      for (const f of props.syncInfo.filesToAdd) {
        const parts = f.path.split('/')
        if (parts.length > 1) dirs.add(parts.slice(0, -1).join('/'))
      }
      for (const f of props.syncInfo.filesToModify) {
        const parts = f.path.split('/')
        if (parts.length > 1) dirs.add(parts.slice(0, -1).join('/'))
      }
      expandedGroups.value = dirs
    }
  },
)

// ─── 操作 ────────────────────────────────────────────────────

function handleApply() {
  emit('apply', [...selectedFiles.value])
}

// ─── 计算属性 ────────────────────────────────────────────────

const dialogTitle = computed(() => {
  switch (props.phase) {
    case 'review':
      return '数据资源同步'
    case 'syncing':
      return '正在同步...'
    case 'complete':
      return '同步完成'
    case 'error':
      return '同步失败'
    default:
      return '数据资源同步'
  }
})

const subtitle = computed(() => {
  if (props.phase === 'review' && props.syncInfo) {
    return `v${props.syncInfo.currentVersion} → v${props.syncInfo.newVersion}`
  }
  return ''
})

// ─── 工具函数 ────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return (bytes / 1_073_741_824).toFixed(1) + ' GB'
  if (bytes >= 1_048_576) return (bytes / 1_048_576).toFixed(1) + ' MB'
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return bytes + ' B'
}
</script>

<style scoped>
/* 过渡动画 */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-active > div,
.modal-leave-active > div {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
.modal-enter-from > div,
.modal-leave-to > div {
  transform: scale(0.95);
  opacity: 0;
}

/* 隐藏滚动条 */
.scrollbar-hide::-webkit-scrollbar {
  display: none;
}
.scrollbar-hide {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
