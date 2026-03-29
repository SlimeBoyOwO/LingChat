<template>
  <MenuPage>
    <MenuItem title="历史对话">
      <template #header>
        <History :size="20" />
      </template>
      <div class="flex flex-col h-full min-h-0">
        <div
          v-if="dialogHistory.length === 0"
          class="text-center text-[#f5f5f5] py-8 text-2xl font-bold h-full"
          style="text-shadow: 0 0 5px rgba(255, 255, 255, 0.5)"
        >
          暂无历史记录，去和ta聊聊天叭(*^▽^*)
        </div>
        <div v-else class="flex flex-col h-full min-h-0">
          <div
            class="p-2.5 flex flex-col gap-3 text-lg flex-1 min-h-0 overflow-y-auto scroll-smooth"
            ref="chatContainer"
          >
            <DialogSession :dialog="currentPageHistory" />
          </div>
          <div class="flex items-center justify-between px-3 py-2 w-full">
            <button
              class="px-4 py-1.5 text-sm font-medium border-none rounded-lg cursor-pointer bg-[#e9ecef] text-[#495057] transition-all duration-200 hover:bg-(--accent-color) hover:text-white hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:opacity-40 disabled:cursor-not-allowed"
              :disabled="currentPage === 1"
              @click="currentPage--"
            >
              上一页
            </button>
            <span class="text-[#f5f5f5] text-base font-medium">
              第 {{ currentPage }} 页 / 共 {{ totalPages }} 页
            </span>
            <button
              class="px-4 py-1.5 text-sm font-medium border-none rounded-lg cursor-pointer bg-[#e9ecef] text-[#495057] transition-all duration-200 hover:bg-(--accent-color) hover:text-white hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:opacity-40 disabled:cursor-not-allowed"
              :disabled="currentPage === totalPages"
              @click="currentPage++"
            >
              下一页
            </button>
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
// 1. 从 vue 中引入 ref 和 watch
import { ref, computed, watch } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { useGameStore } from '../../../stores/modules/game'
import type { GameMessage } from '../../../stores/modules/game/state'
import DialogSession from '../history/DialogSession.vue'
import { History } from 'lucide-vue-next'

const gameStore = useGameStore()

const dialogHistory = computed<GameMessage[]>(() => gameStore.dialogHistory)

// 每页显示的台词数量
const PAGE_SIZE = 100

// 当前页码
const currentPage = ref(1)

// 计算总页数
const totalPages = computed(() => Math.ceil(dialogHistory.value.length / PAGE_SIZE))

// 计算当前页应该显示的对话历史
const currentPageHistory = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return dialogHistory.value.slice(start, end)
})

// 监听对话历史变化，重置到第一页
watch(
  dialogHistory,
  () => {
    currentPage.value = 1
  },
  { deep: true },
)

// 对话初始化逻辑在 gameStore 的初始化中处理
</script>

<style scoped>
/* 所有样式已转换为 Tailwind CSS */
</style>
