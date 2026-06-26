<template>
  <div
    v-if="shouldShow"
    class="fixed top-5 left-5 right-5 z-2000 flex justify-between items-start pointer-events-none"
  >
    <!-- 左侧按钮组：番茄钟、日程、高级 -->
    <div class="flex items-center gap-2 pointer-events-auto">
      <!-- 番茄钟按钮 -->
      <button
        class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
        :class="pomodoroEnabled ? 'text-[#4facfe]' : 'text-white'"
        @click.stop="togglePomodoro"
      >
        <span class="text-xl">🍅</span>
        <span class="hidden md:inline text-lg font-bold">番茄钟 (实验)</span>
      </button>

      <!-- 日程按钮 -->
      <button
        class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
        :class="scheduleEnabled ? 'text-[#4facfe]' : 'text-white'"
        @click.stop="toggleSchedule"
      >
        <Icon icon="schedule" :size="18" />
        <span class="hidden md:inline text-lg font-bold">日程</span>
      </button>

      <!-- 高级功能按钮 -->
      <button
        class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
        :class="advancedEnabled ? 'text-[#4facfe]' : 'text-white'"
        @click.stop="toggleAdvanced"
      >
        <Icon icon="advance" :size="18" />
        <span class="hidden md:inline text-lg font-bold">高级</span>
      </button>
    </div>

    <!-- 右侧按钮组：自动/Code 模式、菜单 -->
    <div class="flex items-center gap-2 pointer-events-auto">
      <!-- Code 模式按钮组 -->
      <template v-if="settingsStore.codeMode">
        <button
          class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
          @click.stop="handleClearHistory"
          title="清空记录"
        >
          <Icon icon="close" :size="18" />
          <span class="hidden md:inline text-lg font-bold">清空</span>
        </button>
        <button
          class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
          @click.stop="handleNewSession"
          title="新建会话"
        >
          <Icon icon="plus" :size="18" />
          <span class="hidden md:inline text-lg font-bold">新建</span>
        </button>
        <button
          class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
          @click.stop="handleContinueSession"
          title="继续上次会话"
        >
          <Icon icon="history" :size="18" />
          <span class="hidden md:inline text-lg font-bold">继续</span>
        </button>
      </template>

      <!-- 普通模式：自动按钮 -->
      <template v-else>
        <button
          class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
          :class="{ active: uiStore.autoMode }"
          @click.stop="switchAutoMode"
        >
          <Icon icon="play" :size="18" />
          <span class="hidden md:inline text-lg font-bold">自动</span>
        </button>
      </template>

      <!-- 菜单按钮 -->
      <button
        class="flex items-center gap-2 px-4 py-2 rounded-lg text-white transition-colors nav-btn"
        @click.stop="openSettings"
      >
        <Icon icon="text" :size="18" />
        <span class="hidden md:inline text-lg font-bold">菜单</span>
      </button>
    </div>
  </div>

  <!-- 下方悬浮面板区域 -->
  <div
    v-if="shouldShow && (pomodoroEnabled || scheduleEnabled || advancedEnabled)"
    class="fixed top-20 left-5 z-1999 flex flex-col gap-3"
  >
    <!-- 番茄钟面板 -->
    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="pomodoroEnabled"
        class="bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-3 text-white"
      >
        <PomodoroContent />
      </div>
    </Transition>

    <!-- 日程面板 - 移动端居中悬浮 -->
    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="scheduleEnabled"
        class="fixed md:absolute left-1/2 md:left-0 top-[80px] md:top-auto -translate-x-1/2 md:translate-x-0 mt-0 md:mt-3 bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-3 text-white"
      >
        <ScheduleContent variant="popup" />
      </div>
    </Transition>

    <!-- 高级功能面板 - 移动端居中悬浮 -->
    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="advancedEnabled"
        class="fixed md:absolute left-1/2 md:left-0 top-[80px] md:top-auto -translate-x-1/2 md:translate-x-0 mt-0 md:mt-3 bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-3 text-white"
      >
        <AdvancedFeaturesContent variant="popup" />
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useGameStore } from '@/stores/modules/game'
import { useSettingsStore } from '@/stores/modules/settings'
import Icon from '@/components/base/widget/Icon.vue'
import PomodoroContent from '@/components/pomodoro/PomodoroContent.vue'
import ScheduleContent from '@/components/schedule/ScheduleContent.vue'
import AdvancedFeaturesContent from '@/components/settings/pages/Advanced/AdvancedFeaturesContent.vue'
import { clearChatHistory } from '@/api/services/history'
import { saveCreate, saveContinue } from '@/api/services/save'

const uiStore = useUIStore()
const gameStore = useGameStore()
const settingsStore = useSettingsStore()

// 面板显示状态
const pomodoroEnabled = ref(false)
const scheduleEnabled = ref(false)
const advancedEnabled = ref(false)

// 剧情模式下不显示
const shouldShow = computed(() => {
  return !(gameStore.runningScript && gameStore.runningScript.isRunning)
})

// 切换面板
function togglePomodoro() {
  pomodoroEnabled.value = !pomodoroEnabled.value
  if (pomodoroEnabled.value) {
    scheduleEnabled.value = false
    advancedEnabled.value = false
  }
}

function toggleSchedule() {
  scheduleEnabled.value = !scheduleEnabled.value
  if (scheduleEnabled.value) {
    pomodoroEnabled.value = false
    advancedEnabled.value = false
  }
}

function toggleAdvanced() {
  advancedEnabled.value = !advancedEnabled.value
  if (advancedEnabled.value) {
    pomodoroEnabled.value = false
    scheduleEnabled.value = false
  }
}

// 右侧按钮功能
function switchAutoMode() {
  uiStore.autoMode = !uiStore.autoMode
}

function openSettings() {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('text')
}

async function handleClearHistory() {
  if (confirm('确定要清空所有对话记录吗？')) {
    await clearChatHistory()
  }
}

async function handleNewSession() {
  if (confirm('确定要新建会话吗？')) {
    await saveCreate()
  }
}

async function handleContinueSession() {
  await saveContinue()
}

// 打开设置时关闭所有面板
watch(
  () => uiStore.showSettings,
  (show) => {
    if (show) {
      pomodoroEnabled.value = false
      scheduleEnabled.value = false
      advancedEnabled.value = false
    }
  }
)
</script>

<style scoped>
.nav-btn {
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.nav-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.nav-btn.active {
  background: rgba(79, 172, 254, 0.3);
  border-color: #4facfe;
}
</style>
