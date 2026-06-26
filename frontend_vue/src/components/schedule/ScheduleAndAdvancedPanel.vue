<template>
  <div class="flex flex-col gap-3" data-wheel-history-ignore @wheel.stop>
    <!-- 按钮组：番茄钟旁边显示日程和高级功能 -->
    <div class="flex items-center gap-2">
      <Button
        type="nav"
        icon="schedule"
        :class="[
          'flex items-center gap-2 px-4 py-2 transition-colors',
          scheduleEnabled ? 'text-[#4facfe]' : 'text-white',
        ]"
        @click="toggleSchedule"
        v-show="!uiStore.showSettings"
      >
        <h3 class="text-lg font-bold m-0 hidden md:inline">日程</h3>
      </Button>
      <Button
        type="nav"
        icon="advance"
        :class="[
          'flex items-center gap-2 px-4 py-2 transition-colors',
          advancedEnabled ? 'text-[#4facfe]' : 'text-white',
        ]"
        @click="toggleAdvanced"
        v-show="!uiStore.showSettings"
      >
        <h3 class="text-lg font-bold m-0 hidden md:inline">高级</h3>
      </Button>
    </div>
  </div>

  <!-- 日程面板 - 移动端悬浮居中，桌面端相对定位 -->
  <Transition
    enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
    leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
    enter-from-class="opacity-0 -translate-y-2"
    leave-to-class="opacity-0 -translate-y-2"
  >
    <div
      v-if="scheduleEnabled"
      class="fixed md:absolute left-1/2 md:left-0 top-[100px] md:top-auto -translate-x-1/2 md:translate-x-0 mt-0 md:mt-3 z-[1999] md:z-auto bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-3 text-white box-border"
      data-wheel-history-ignore
      @wheel.stop
    >
      <ScheduleContent variant="popup" />
    </div>
  </Transition>

  <!-- 高级功能面板 - 移动端悬浮居中，桌面端相对定位 -->
  <Transition
    enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
    leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
    enter-from-class="opacity-0 -translate-y-2"
    leave-to-class="opacity-0 -translate-y-2"
  >
    <div
      v-if="advancedEnabled"
      class="fixed md:absolute left-1/2 md:left-0 top-[100px] md:top-auto -translate-x-1/2 md:translate-x-0 mt-0 md:mt-3 z-[1999] md:z-auto bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-3 text-white box-border"
      data-wheel-history-ignore
      @wheel.stop
    >
      <AdvancedFeaturesContent variant="popup" />
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import Button from '@/components/base/widget/Button.vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import ScheduleContent from './ScheduleContent.vue'
import AdvancedFeaturesContent from '@/components/settings/pages/Advanced/AdvancedFeaturesContent.vue'

const uiStore = useUIStore()

const scheduleEnabled = ref(false)
const advancedEnabled = ref(false)

function toggleSchedule() {
  scheduleEnabled.value = !scheduleEnabled.value
  if (scheduleEnabled.value) {
    advancedEnabled.value = false
  }
}

function toggleAdvanced() {
  advancedEnabled.value = !advancedEnabled.value
  if (advancedEnabled.value) {
    scheduleEnabled.value = false
  }
}

watch(
  () => uiStore.showSettings,
  (show) => {
    if (show) {
      scheduleEnabled.value = false
      advancedEnabled.value = false
    }
  },
)
</script>
