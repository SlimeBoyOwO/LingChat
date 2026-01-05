<template>
  <MenuPage>
    <div
      class="w-full max-w-6xl h-[87vh] glass-panel bg-white/10 rounded-2xl overflow-hidden flex flex-col md:flex-row"
    >
      <!-- 导航菜单 (左侧) -->
      <aside class="w-full md:w-64 p-6 flex flex-col border-r border-cyan-300">
        <div
          class="flex items-center space-x-3 text-base font-bold px-3.75 py-2.5 rounded-lg mb-8 text-brand inset_0_1px_1px_rgba(255,255,255,0.1)]"
        >
          <div
            class="w-10 h-10 bg-cyan-500 rounded-xl flex items-center justify-center text-white shadow-lg"
          >
            <Sparkles :size="20" />
          </div>
          <h1 class="font-bold text-xl text-white tracking-tight">LingChat AI</h1>
        </div>

        <nav class="flex-1 space-y-2 w-full">
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
            @click="changeView('schedule_groups')"
          >
            <Layers :size="18" />
            <span>日程主题</span>
          </button>
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
            @click="changeView('todo_groups')"
          >
            <CheckCircle2 :size="18" />
            <span>待办事项</span>
          </button>
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
            @click="changeView('calendar')"
          >
            <CalendarDays :size="18" />
            <span>重要日子</span>
          </button>
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          >
            <Cat :size="18" />
            <span>主动对话</span>
          </button>
        </nav>

        <div class="mt-auto mb-6 p-4 bg-cyan-50/10 rounded-2xl border border-cyan-500/20">
          <div class="flex items-center text-brand font-bold text-xs mb-2">
            <span class="w-2 h-2 bg-cyan-500 rounded-full animate-pulse mr-2"></span>
            Ling Clock
          </div>
          <p class="text-xs text-white italic leading-relaxed">
            "在这里添加的信息屏幕后的那个ta也看得到哦！"
          </p>
        </div>
      </aside>

      <main class="flex-1 flex flex-col overflow-hidden">
        <header class="mt-2 p-6 flex justify-between items-center border-b border-cyan-300">
          <div class="flex items-center space-x-4 pl-4">
            <button
              v-show="uiStore.scheduleView === 'schedule_detail'"
              @click="uiStore.scheduleView = 'schedule_groups'"
              class="p-2 hover:bg-cyan-50 rounded-full text-cyan-600 transition-all"
            >
              <i data-lucide="chevron-left"></i>
              <ChevronLeft />
            </button>
            <div>
              <h2 class="text-2xl font-bold text-brand mb-2">小灵闹钟</h2>
              <p class="text-xs text-white mt-0.5 tracking-wide">留下需要她提醒你的事情吧</p>
            </div>
          </div>

          <button
            @click="openModal"
            class="bg-cyan-500 hover:bg-cyan-600 text-white px-5 py-2.5 rounded-xl shadow-lg transition-all flex items-center space-x-2"
          >
            <Plus></Plus>
            <span class="font-medium">新建</span>
          </button>
        </header>

        <!-- 内容滚动容器 -->
        <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
          <!--日程界面-->
          <SchedulePage />

          <!--待办事项界面-->
          <TodoPage />

          <!--日历页面-->
          <CalendarPage />
        </div>
      </main>
    </div>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage } from '../../ui'
import { useUIStore } from '@/stores/modules/ui/ui'
import TodoPage from './Schedule/TodoPage.vue'
import SchedulePage from './Schedule/SchedulePage.vue'
import CalendarPage from './Schedule/CalendarPage.vue'
import {
  Layers,
  CheckCircle2,
  CalendarDays,
  Plus,
  Cat,
  ChevronLeft,
  Sparkles,
} from 'lucide-vue-next'

const uiStore = useUIStore()

const openModal = () => {}

const changeView = (view: string) => {
  uiStore.scheduleView = view
}
</script>
