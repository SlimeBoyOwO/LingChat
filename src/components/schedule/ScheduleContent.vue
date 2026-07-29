<template>
  <div class="w-full flex-1 overflow-hidden flex flex-col md:flex-row" :class="containerClass">
    <!-- 导航菜单 (左侧)：宽屏始终可见；窄屏仅在浏览菜单层级时可见 -->
    <aside
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'menu'"
      class="w-full md:w-64 p-6 flex flex-col border-r border-cyan-300"
      :class="{ 'flex-1 min-h-0': uiStore.isNarrowScreen }"
    >
      <div
        class="flex items-center space-x-3 text-base font-bold px-3.75 py-2.5 rounded-lg mb-8 text-brand inset_0_1px_1px_rgba(255,255,255,0.1)]"
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

      <nav class="flex-1 min-h-0 overflow-y-auto space-y-2 w-full">
        <button
          class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          @click="changeView('schedule_groups')"
        >
          <Layers :size="18" />
          <span>{{ $t('ui.scheduleContent.navSchedule') }}</span>
        </button>
        <button
          class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          @click="changeView('todo_groups')"
        >
          <CheckCircle2 :size="18" />
          <span>{{ $t('ui.scheduleContent.navTodo') }}</span>
        </button>
        <button
          class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          @click="changeView('calendar')"
        >
          <CalendarDays :size="18" />
          <span>{{ $t('ui.scheduleContent.navCalendar') }}</span>
        </button>
        <button
          class="w-full flex items-center space-x-6 px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          @click="changeView('proactive_settings')"
        >
          <Cat :size="18" />
          <span>{{ $t('ui.scheduleContent.navProactive') }}</span>
        </button>
      </nav>

      <div class="mt-auto mb-6 p-4 bg-cyan-50/10 rounded-2xl border border-cyan-500/20">
        <div class="flex items-center text-brand font-bold text-xs mb-2">
          <span class="w-2 h-2 bg-cyan-500 rounded-full animate-pulse mr-2"></span>
          Ling Clock
        </div>
        <p class="text-xs text-white italic leading-relaxed">
          {{ $t('ui.scheduleContent.clockTip') }}
        </p>
      </div>
    </aside>

    <main
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'content'"
      class="flex-1 flex flex-col overflow-hidden w-full"
    >
      <header
        class="flex justify-between items-center border-b border-cyan-300 shrink-0"
        :class="uiStore.isNarrowScreen ? 'px-3 py-3' : 'mt-2 p-6'"
      >
        <div
          class="flex items-center min-w-0"
          :class="uiStore.isNarrowScreen ? 'space-x-2' : 'space-x-4 pl-4'"
        >
          <!-- 窄屏：返回菜单按钮 -->
          <button
            v-if="uiStore.isNarrowScreen"
            @click="narrowViewLevel = 'menu'"
            class="flex items-center gap-1 text-sm text-white/70 hover:text-white transition-colors py-1 px-1.5 rounded-lg hover:bg-white/10 shrink-0"
          >
            <ChevronLeft :size="18" />
          </button>
          <!-- 宽屏：返回上级视图（详情 → 分组） -->
          <button
            v-show="
              !uiStore.isNarrowScreen &&
              (uiStore.scheduleView === 'schedule_detail' || uiStore.scheduleView === 'todo_detail')
            "
            @click="goBackToParentView"
            class="p-2 hover:bg-cyan-50 rounded-full text-cyan-600 transition-all"
          >
            <ChevronLeft />
          </button>
          <div class="min-w-0">
            <h2
              class="font-bold text-brand truncate"
              :class="uiStore.isNarrowScreen ? 'text-base' : 'text-2xl mb-2'"
            >
              {{ titleInfo.title }}
            </h2>
            <p v-show="!uiStore.isNarrowScreen" class="text-xs text-white mt-0.5 tracking-wide">
              {{ titleInfo.subtitle }}
            </p>
          </div>
        </div>

        <button
          v-show="!uiStore.scheduleView.startsWith('proactive')"
          @click="triggerCreate"
          class="bg-cyan-500 hover:bg-cyan-600 text-white rounded-xl shadow-lg transition-all flex items-center shrink-0"
          :class="uiStore.isNarrowScreen ? 'px-3 py-2 text-sm space-x-1' : 'px-5 py-2.5 space-x-2'"
        >
          <Plus :size="uiStore.isNarrowScreen ? 16 : undefined" />
          <span class="font-medium" :class="{ hidden: uiStore.isNarrowScreen }">{{ $t('ui.scheduleContent.create') }}</span>
        </button>
      </header>

      <!-- 内容滚动容器 -->
      <div
        class="flex-1 overflow-y-auto custom-scrollbar"
        :class="uiStore.isNarrowScreen ? 'p-3' : 'p-6'"
      >
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
import { useI18n } from 'vue-i18n'
import { useUIStore } from '@/stores/modules/ui/ui'
import TodoPage from '@/components/schedule/pages/TodoPage.vue'
import SchedulePage from '@/components/schedule/pages/SchedulePage.vue'
import CalendarPage from '@/components/schedule/pages/CalendarPage.vue'
import ProactivePage from '@/components/schedule/pages/ProactivePage.vue'
import {
  Layers,
  CheckCircle2,
  CalendarDays,
  Plus,
  Cat,
  ChevronLeft,
  Sparkles,
} from 'lucide-vue-next'

type Variant = 'settings' | 'popup'

const props = withDefaults(
  defineProps<{
    variant?: Variant
  }>(),
  { variant: 'settings' },
)

const uiStore = useUIStore()
const { t } = useI18n()
const narrowViewLevel = ref<'menu' | 'content'>('menu')

const scheduleRef = ref()
const todoRef = ref()
const calendarRef = ref()
const titleInfo = computed(() => {
  const currentView = uiStore.scheduleView

  if (currentView.startsWith('schedule')) {
    return {
      title: t('ui.scheduleContent.titleSchedule'),
      subtitle: t('ui.scheduleContent.subtitleSchedule'),
    }
  } else if (currentView.startsWith('todo')) {
    return {
      title: t('ui.scheduleContent.titleTodo'),
      subtitle: t('ui.scheduleContent.subtitleTodo'),
    }
  } else if (currentView.startsWith('proactive')) {
    return {
      title: t('ui.scheduleContent.titleProactive'),
      subtitle: t('ui.scheduleContent.subtitleProactive'),
    }
  } else if (currentView.startsWith('calendar')) {
    return {
      title: t('ui.scheduleContent.titleCalendar'),
      subtitle: t('ui.scheduleContent.subtitleCalendar'),
    }
  } else {
    // 默认情况
    return {
      title: t('ui.scheduleContent.titleDefault'),
      subtitle: t('ui.scheduleContent.subtitleDefault'),
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

const changeView = (view: string) => {
  uiStore.scheduleView = view
  // 窄屏下自动切换到内容视图
  if (uiStore.isNarrowScreen) {
    narrowViewLevel.value = 'content'
  }
}

const goBackToParentView = () => {
  if (uiStore.scheduleView === 'schedule_detail') {
    uiStore.scheduleView = 'schedule_groups'
  } else if (uiStore.scheduleView === 'todo_detail') {
    uiStore.scheduleView = 'todo_groups'
  }
}

const containerClass = computed(() => {
  // settings：沿用原来的全屏设置页布局
  if (props.variant === 'settings') {
    return 'h-[85vh] max-w-6xl md:w-[calc(100vw-4rem)] glass-panel bg-white/10 rounded-2xl'
  }
  // popup：由父级 modal 控制尺寸和样式，此处填满容器
  return 'w-full h-full'
})
</script>
