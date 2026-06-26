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
          :class="{ 'bg-cyan-500/30 font-bold': currentView === item.key }"
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
          AI 实验室
        </div>
        <p class="text-xs text-white italic leading-relaxed">
          "这里是 AI 的高级功能区域，包含工具记录、记忆库和代码沙盒"
        </p>
      </div>
    </aside>

    <main class="flex-1 flex flex-col overflow-hidden min-w-0">
      <header
        class="mt-2 p-3 md:p-6 flex justify-between items-center border-b border-cyan-300 gap-2"
      >
        <div class="flex items-center space-x-2 md:space-x-4 pl-1 md:pl-4 min-w-0">
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
        <ToolCallsPage ref="toolCallsRef" :view="currentView" />
        <MemoryPage ref="memoryRef" :view="currentView" />
        <SandboxPage ref="sandboxRef" :view="currentView" />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { PawPrint, Sparkles, Wrench, Brain, Shield, Plus } from 'lucide-vue-next'
import ToolCallsPage from './ToolCallsPage.vue'
import MemoryPage from './MemoryPage.vue'
import SandboxPage from './SandboxPage.vue'

type AdvancedViewType = 'tool_calls' | 'memories' | 'sandbox'

type Variant = 'settings' | 'popup'

const props = withDefaults(
  defineProps<{
    variant?: Variant
  }>(),
  { variant: 'settings' },
)

const currentView = ref<AdvancedViewType>('tool_calls')
const memoryRef = ref()
const toolCallsRef = ref()
const sandboxRef = ref()

// 导航项配置
const navItems = [
  { key: 'tool_calls', label: '工具记录', icon: Wrench },
  { key: 'memories', label: '记忆库', icon: Brain },
  { key: 'sandbox', label: '代码沙盒', icon: Shield },
]

const canCreate = computed(() => {
  return currentView.value === 'memories'
})

const titleInfo = computed(() => {
  switch (currentView.value) {
    case 'tool_calls':
      return {
        title: '工具调用记录',
        subtitle: '查看 LingChat AI 最近使用过的内部工具',
      }
    case 'memories':
      return {
        title: '记忆库',
        subtitle: '像 Codex 记忆一样，保存希望 AI 以后参考的要点',
      }
    case 'sandbox':
      return {
        title: '代码沙盒',
        subtitle: 'AI 可安全读写文件和执行命令的工作空间',
      }
    default:
      return {
        title: 'AI 实验室',
        subtitle: '高级功能设置',
      }
  }
})

const triggerCreate = () => {
  if (currentView.value === 'memories') {
    memoryRef.value?.handleCreate()
  }
}

const changeView = (view: AdvancedViewType) => {
  currentView.value = view
}

const containerClass = computed(() => {
  if (props.variant === 'settings') {
    return 'h-[85vh] max-w-6xl md:w-[calc(100vw-4rem)]'
  }
  // 移动端 popup：限制最大宽度为视口的 85%，桌面端保持原尺寸
  return 'h-[70vh] w-[calc(100vw-80px)] md:w-[820px] max-w-[85vw] md:max-w-[90vw]'
})
</script>
