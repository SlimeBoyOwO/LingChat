<template>
  <div class="flex flex-col gap-3">
    <Button
      type="nav"
      icon="schedule"
      :class="[
        'flex items-center gap-2 px-4 py-2 transition-colors',
        enabled ? 'text-[#4facfe]' : 'text-white',
      ]"
      @click="toggleEnabled"
      v-show="!uiStore.showSettings"
    >
      <h3 class="text-lg font-bold m-0">日程</h3>
    </Button>

    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="enabled"
        class="bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-3 text-white box-border"
      >
        <ScheduleContent variant="popup" />
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import Button from '@/components/base/widget/Button.vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import ScheduleContent from './ScheduleContent.vue'

const uiStore = useUIStore()

const enabled = ref(false)

function toggleEnabled() {
  enabled.value = !enabled.value
}

watch(
  () => uiStore.showSettings,
  (show) => {
    if (show) enabled.value = false
  },
)
</script>
