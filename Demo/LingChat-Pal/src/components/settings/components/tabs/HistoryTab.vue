<template>
  <article class="w-full h-full flex flex-col min-h-0">
    <!-- 头部区域 (复用原视觉风格) -->
    <header class="mb-6 flex items-end justify-between border-b-2 pb-2 transition-colors shrink-0"
      :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'">
      <div>
        <h2 class="text-xl font-black tracking-wide mb-1 transition-colors flex items-center gap-2"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'">
          <History class="w-5 h-5" />
          历史对话
        </h2>
        <p class="text-xs font-medium transition-colors" :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'">
          回顾与ta的过往交流记录吧~
        </p>
      </div>
      <span class="text-4xl font-bold italic select-none font-mono transition-colors"
        :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'">
        02
      </span>
    </header>

    <!-- 主体内容区域 -->
    <div class="flex flex-col flex-1 min-h-0 gap-3">
      <!-- 空状态展示 -->
      <div v-if="dialogHistory.length === 0"
        class="flex-1 flex flex-col items-center justify-center p-8 rounded-xl border-2 border-dashed transition-all"
        :class="isDarkMode
          ? 'bg-slate-800/30 border-slate-700 text-slate-500'
          : 'bg-slate-50 border-slate-200 text-slate-400'
          ">
        <MessageSquare class="w-12 h-12 mb-4 opacity-50" />
        <p class="text-sm font-bold tracking-wider">
          暂无历史记录，去和ta聊聊天叭(*^▽^*)
        </p>
      </div>

      <!-- 历史记录列表 -->
      <div v-else class="flex flex-col flex-1 min-h-0 gap-4">
        <!-- 滚动对话区域 -->
        <div class="flex-1 min-h-0 overflow-y-auto p-4 rounded-xl border shadow-sm transition-all" :class="isDarkMode
          ? 'bg-slate-800/50 border-slate-700'
          : 'bg-white border-slate-200'
          " ref="chatContainer">
          <!-- 引入历史对话组件 -->
          <DialogSession :dialog="currentPageHistory" />
        </div>

        <!-- 分页控制器 -->
        <div v-if="totalPages > 1" class="flex items-center justify-between px-1 shrink-0">
          <button
            class="px-4 py-2 text-xs font-bold rounded-lg transition-all flex items-center gap-1 border cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
            :class="isDarkMode
              ? 'bg-slate-800/50 text-slate-300 border-slate-700 hover:bg-slate-700 hover:border-slate-600 hover:text-sky-400'
              : 'bg-white text-slate-600 border-slate-200 hover:bg-sky-50 hover:border-sky-200 hover:text-sky-500'
              " :disabled="currentPage === 1" @click="currentPage--">
            <ChevronLeft class="w-4 h-4" /> 上一页
          </button>

          <span class="text-xs font-bold tracking-widest font-mono transition-colors"
            :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'">
            PAGE {{ currentPage }} / {{ totalPages }}
          </span>

          <button
            class="px-4 py-2 text-xs font-bold rounded-lg transition-all flex items-center gap-1 border cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
            :class="isDarkMode
              ? 'bg-slate-800/50 text-slate-300 border-slate-700 hover:bg-slate-700 hover:border-slate-600 hover:text-sky-400'
              : 'bg-white text-slate-600 border-slate-200 hover:bg-sky-50 hover:border-sky-200 hover:text-sky-500'
              " :disabled="currentPage === totalPages" @click="currentPage++">
            下一页
            <ChevronRight class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import {
  History,
  MessageSquare,
  ChevronLeft,
  ChevronRight,
} from "lucide-vue-next";

// --- 引入原有的 Store 和 组件 ---
import { useGameStore } from "../../../../stores/modules/game";
import type { GameMessage } from "../../../../stores/modules/game/state";
import DialogSession from "../history/DialogSession.vue";

// --- Props ---
defineProps<{
  isDarkMode: boolean;
}>();

// --- 状态与逻辑 ---
const gameStore = useGameStore();

const dialogHistory = computed<GameMessage[]>(() => gameStore.dialogHistory);
const chatContainer = ref<HTMLElement | null>(null);

// 每页显示的台词数量
const PAGE_SIZE = 100;

// 当前页码
const currentPage = ref(1);

// 计算总页数
const totalPages = computed(() =>
  Math.ceil(dialogHistory.value.length / PAGE_SIZE),
);

// 计算当前页应该显示的对话历史
const currentPageHistory = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE;
  const end = start + PAGE_SIZE;
  return dialogHistory.value.slice(start, end);
});

// 监听对话历史变化，重置到第一页，并滚动到顶部

// 切换页码时，容器自动滚动回顶部
watch(currentPage, () => {
  if (chatContainer.value) {
    chatContainer.value.scrollTop = 0;
  }
});
</script>
