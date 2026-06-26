<template>
  <div
    class="absolute -top-1 -left-2 w-10 h-10 rounded-full flex items-center justify-center text-brand shadow-md transform -rotate-18 md:block hidden"
  >
    <PawPrint :size="58" />
  </div>
  <div
    class="w-full flex-1 glass-panel bg-white/10 rounded-2xl overflow-hidden flex flex-col md:flex-row"
    :class="containerClass"
  >
    <!-- 导航菜单 - 桌面端左侧，移动端顶部水平 -->
    <aside class="w-full md:w-64 p-4 md:p-6 flex flex-col border-r border-cyan-300 md:border-r">
      <!-- Logo 区域 - 桌面端显示 -->
      <div
        class="hidden md:flex items-center space-x-3 text-base font-bold px-3.75 py-2.5 rounded-lg mb-8 text-brand inset_0_1px_1px_rgba(255,255,255,0.1)]"
      >
        <div class="relative">
          <div
            class="w-10 h-10 bg-cyan-500 rounded-xl flex items-center justify-center text-white shadow-lg"
          >
            <Sparkles :size="20" />
          </div>
        </div>
        <h1 class="font-bold text-xl text-white tracking-tight">LingChat AI</h1>
      </div>

      <!-- 导航选项卡 - 移动端水平排列，间距更紧凑 -->
      <nav class="flex-1 w-full md:space-y-2 flex md:flex-col gap-1 md:gap-0 overflow-x-auto">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="flex items-center justify-center md:justify-start md:space-x-6 px-2 py-1.5 md:px-5 md:py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black whitespace-nowrap text-sm md:text-base"
          :class="{ 'bg-cyan-500/30 font-bold': uiStore.scheduleView === item.key }"
          @click="changeView(item.key)"
        >
          <component :is="item.icon" :size="16" />
          <span class="md:ml-6 ml-1.5">{{ item.label }}</span>
        </button>
      </nav>

      <!-- 底部提示 - 桌面端显示 -->
      <div
        class="hidden md:block mt-auto mb-6 p-4 bg-cyan-50/10 rounded-2xl border border-cyan-500/20"
      >
        <div class="flex items-center text-brand font-bold text-xs mb-2">
          <span class="w-2 h-2 bg-cyan-500 rounded-full animate-pulse mr-2"></span>
          Ling Clock
        </div>
        <p class="text-xs text-white italic leading-relaxed">
          "在这里添加的信息屏幕后的那个ta也看得到哦！"
        </p>
      </div>
    </aside>

    <main class="flex-1 flex flex-col overflow-hidden min-w-0">
      <header
        class="mt-2 p-3 md:p-6 flex justify-between items-center border-b border-cyan-300 gap-2"
      >
        <div class="flex items-center space-x-2 md:space-x-4 pl-1 md:pl-4 min-w-0">
          <button
            v-show="
              uiStore.scheduleView === 'schedule_detail' || uiStore.scheduleView === 'todo_detail'
            "
            @click="
              uiStore.scheduleView =
                uiStore.scheduleView === 'schedule_detail' ? 'schedule_groups' : 'todo_groups'
            "
            class="shrink-0 p-2 hover:bg-cyan-50 rounded-full text-cyan-600 transition-all"
          >
            <ChevronLeft />
          </button>
          <div class="min-w-0">
            <h2 class="text-base md:text-2xl font-bold text-brand mb-0.5 md:mb-2 truncate">
              {{ titleInfo.title }}
            </h2>
            <p class="text-xs text-white mt-0.5 tracking-wide hidden md:block">
              {{ titleInfo.subtitle }}
            </p>
          </div>
        </div>

        <button
          v-if="canCreate"
          @click="triggerCreate"
          class="shrink-0 bg-cyan-500 hover:bg-cyan-600 text-white px-2 py-1.5 md:px-5 md:py-2.5 rounded-xl shadow-lg transition-all flex items-center space-x-1 md:space-x-2"
        >
          <Plus :size="14" class="md:hidden"></Plus>
          <Plus class="hidden md:block"></Plus>
          <span class="font-medium hidden md:inline">新建</span>
          <span class="font-medium md:hidden">+</span>
        </button>
      </header>

      <!-- 内容滚动容器 -->
      <div class="flex-1 overflow-y-auto p-3 md:p-6 custom-scrollbar">
        <!--日程界面-->
        <SchedulePage ref="scheduleRef" />

        <!--待办事项界面-->
        <TodoPage ref="todoRef" />

        <!--日历页面-->
        <CalendarPage ref="calendarRef" />

        <ProactivePage ref="proactiveRef" />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useUIStore, type ScheduleViewType } from '@/stores/modules/ui/ui'
import TodoPage from '@/components/settings/pages/Schedule/TodoPage.vue'
import SchedulePage from '@/components/settings/pages/Schedule/SchedulePage.vue'
import CalendarPage from '@/components/settings/pages/Schedule/CalendarPage.vue'
import ProactivePage from '../settings/pages/Schedule/ProactivePage.vue'
import {
  Layers,
  CheckCircle2,
  CalendarDays,
  Plus,
  Cat,
  ChevronLeft,
  Sparkles,
  PawPrint,
} from 'lucide-vue-next'

type Variant = 'settings' | 'popup'

const props = withDefaults(
  defineProps<{
    variant?: Variant
  }>(),
  { variant: 'settings' },
)

const scheduleRef = ref()
const todoRef = ref()
const calendarRef = ref()
const proactiveRef = ref()

const uiStore = useUIStore()

// 导航项配置
const navItems = [
  { key: 'schedule_groups', label: '日程主题', icon: Layers },
  { key: 'todo_groups', label: '待办事项', icon: CheckCircle2 },
  { key: 'calendar', label: '重要日子', icon: CalendarDays },
  { key: 'proactive_settings', label: '主动对话', icon: Cat },
]

const canCreate = computed(() => {
  return !['proactive_settings'].includes(uiStore.scheduleView)
})

const titleInfo = computed(() => {
  const currentView = uiStore.scheduleView

  if (currentView.startsWith('schedule')) {
    return {
      title: '铃铃提醒闹钟',
      subtitle: '到点的时候ta会提醒你的哦',
    }
  } else if (currentView.startsWith('todo')) {
    return {
      title: 'TODO 待办笔记',
      subtitle: '在这里记录重要的事情吧，ta会随机提醒你哒',
    }
  } else if (currentView.startsWith('proactive')) {
    return {
      title: '主动对话设置',
      subtitle: '需要专心和隐私的时候可以关闭哦（需要点击底部的保存才生效）',
    }
  } else if (currentView.startsWith('calendar')) {
    return {
      title: '君の重要な日',
      subtitle: '可以记下你朋友的生日自动提醒哦',
    }
  } else {
    // 默认情况
    return {
      title: '小灵闹钟',
      subtitle: '留下需要她提醒你的事情吧',
    }
  }
})

const triggerCreate = () => {
  const currentView = uiStore.scheduleView

  // 这里的逻辑是：判断当前在哪个视图，就调用哪个组件内部的 handleCreate 方法
  if (currentView.startsWith('schedule')) {
    // 日程相关视图
    scheduleRef.value?.handleCreate()
  } else if (currentView.startsWith('todo')) {
    // 待办相关视图
    todoRef.value?.handleCreate()
  } else if (currentView === 'calendar') {
    // 日历视图
    calendarRef.value?.handleCreate()
  }
}

const changeView = (view: ScheduleViewType) => {
  uiStore.scheduleView = view
}

const containerClass = computed(() => {
  // settings：沿用原来的全屏设置页布局
  if (props.variant === 'settings') {
    return 'h-[85vh] max-w-6xl md:w-[calc(100vw-4rem)]'
  }
  // popup：移动端限制最大宽度为视口的 85%，桌面端保持原尺寸
  return 'h-[70vh] w-[calc(100vw-80px)] md:w-[820px] max-w-[85vw] md:max-w-[90vw]'
})
</script>
