<template>
  <MenuPage>
    <div
      class="w-full max-w-6xl h-[87vh] glass-panel bg-white/10 rounded-lg overflow-hidden flex flex-col md:flex-row"
    >
      <!-- 导航菜单 (左侧) -->
      <aside class="w-full md:w-64 p-6 flex flex-col border-r border-cyan-300">
        <div
          class="flex items-center space-x-3 text-base font-bold px-3.75 py-2.5 rounded-lg mb-8 text-brand inset_0_1px_1px_rgba(255,255,255,0.1)]"
        >
          <div
            class="w-10 h-10 bg-cyan-500 rounded-xl flex items-center justify-center text-white shadow-lg"
          >
            <LayoutDashboard :size="20" />
          </div>
          <h1 class="font-bold text-xl text-white tracking-tight">LingChat AI</h1>
        </div>

        <nav class="flex-1 space-y-2 w-full">
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          >
            <Layers :size="18" />
            <span>日程主题</span>
          </button>
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          >
            <CheckCircle2 :size="18" />
            <span>待办事项</span>
          </button>
          <button
            class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
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
              v-show="currentView === 'schedule_detail'"
              @click="currentView = 'schedule_groups'"
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
          <!-- 视图：日程主题列表 -->
          <div
            v-if="currentView === 'schedule_groups'"
            class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-2 gap-6"
          >
            <div
              v-for="(group, id) in scheduleGroups"
              :key="id"
              @click="selectGroup(id)"
              class="group glass-effect p-6 rounded-3xl border border-brand shadow-sm hover:shadow-xl hover:-translate-y-1 transition-all cursor-pointer"
            >
              <div
                class="w-12 h-12 bg-cyan-500 rounded-2xl flex items-center justify-center text-cyan-50 mb-4 group-hover:bg-cyan-50 group-hover:text-cyan-500 transition-colors"
              >
                <FolderKanban></FolderKanban>
              </div>
              <h3 class="font-bold text-lg text-brand">
                {{ group.title }}
              </h3>
              <p class="text-sm text-white mt-2 line-clamp-2">
                {{ group.description }}
              </p>
              <div
                class="mt-6 pt-4 border-t border-slate-50 flex justify-between items-center text-xs font-bold text-brand"
              >
                <span>{{ group.items.length }} 个日程</span>
                <ArrowRight :size="16" />
              </div>
            </div>
          </div>

          <!-- 视图：日程详情列表 -->
          <div v-if="currentView === 'schedule_detail'" class="max-w-3xl mx-auto space-y-4">
            <div
              v-for="(item, idx) in activeGroup.items"
              :key="idx"
              class="glass-effect p-5 rounded-2xl border border-slate-100 shadow-sm flex items-start space-x-4"
            >
              <div
                class="bg-cyan-500 text-white px-3 py-1 rounded-lg text-xs font-bold self-center"
              >
                {{ item.time }}
              </div>
              <div class="flex-1">
                <h4 class="font-bold text-brand text-lg">{{ item.name }}</h4>
                <p class="text-sm text-white mt-1">{{ item.content }}</p>
              </div>
              <button
                @click="removeScheduleItem(idx)"
                class="text-slate-300 hover:text-red-400 p-1"
              >
                <Trash2 />
              </button>
            </div>
          </div>
        </div>
      </main>
    </div>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage, MenuItem } from '../../ui'
import { Toggle } from '../../base'
import { ref, computed } from 'vue'
import { LayoutDashboard } from 'lucide-vue-next'
import {
  Layers,
  CheckCircle2,
  CalendarDays,
  Plus,
  Cat,
  ArrowRight,
  Trash2,
  FolderKanban,
  ChevronLeft,
} from 'lucide-vue-next'

const openModal = () => {}
const removeScheduleItem = () => {}

const currentView = ref('schedule_groups')
const selectedGroupId = ref<string | null>(null)

const selectGroup = (id: string) => {
  selectedGroupId.value = id
  currentView.value = 'schedule_detail'
}

// 数据存储
const scheduleGroups = ref({
  g1: {
    title: '莱姆的日常生活',
    description: '涵盖了每日的起床、工作、休息提醒。',
    items: [
      { name: '早起问候', time: '12:00', content: '问候莱姆起床' },
      { name: '午休结束', time: '14:00', content: '提醒写代码' },
    ],
  },
  g2: {
    title: '期末复习周',
    description: '12月考试冲刺期间的特殊时间分配。',
    items: [],
  },
})

const todos = ref([
  {
    id: 1,
    text: '学习日语：N2 核心词汇',
    priority: 5,
    completed: false,
    created: '2025-10-24 10:00',
    doneAt: null,
  },
  {
    id: 2,
    text: '更新 LingChat 前端代码',
    priority: 4,
    completed: true,
    created: '2025-10-23 09:00',
    doneAt: '2025-10-23 15:30',
  },
])

const importantDays = ref([
  { id: 'e1', date: '2025-11-09', title: '莱姆生日' },
  { id: 'e2', date: '2025-01-04', title: 'Slary 生日' },
])

const activeGroup = computed(() => {
  if (!selectedGroupId.value) {
    return { items: [] }
  }
  return scheduleGroups.value[selectedGroupId.value] || { items: [] }
})
</script>
