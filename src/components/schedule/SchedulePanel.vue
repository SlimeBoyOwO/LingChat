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
      <h3 class="text-lg font-bold m-0 hidden xl:block">日程</h3>
    </Button>

    <!-- Modal overlay -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
        leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="enabled"
          class="fixed inset-0 z-[1100] flex items-center justify-center bg-black/50 backdrop-blur-sm"
        >
          <Transition
            enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
            leave-active-class="transition-all duration-200 cubic-bezier(0.6, -0.28, 0.74, 0.05)"
            enter-from-class="opacity-0 scale-95 translate-y-2"
            leave-to-class="opacity-0 scale-95 translate-y-2"
          >
            <div
              v-if="enabled"
              class="relative flex flex-col rounded-3xl border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)]"
              :class="uiStore.isNarrowScreen ? 'w-[95vw] h-[85vh]' : 'w-[80vw] h-[80vh] max-w-[1200px]'"
            >
              <!-- Header bar -->
              <div class="flex items-center justify-between shrink-0 px-5 py-3 bg-[#12121c]/90 backdrop-blur-xl border-b border-white/10">
                <div class="flex items-center gap-2">
                  <PawPrint :size="24" class="text-brand -rotate-18" />
                  <h3 class="text-white text-base font-semibold">日程</h3>
                </div>
                <button
                  class="p-2 rounded-full text-white/50 hover:text-white hover:bg-white/10 transition-colors"
                  @click="enabled = false"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                </button>
              </div>

              <!-- Content area with glass styling -->
              <div class="flex-1 min-h-0 bg-[#12121c]/75 backdrop-blur-[20px] rounded-b-3xl overflow-hidden">
                <ScheduleContent variant="popup" />
              </div>
            </div>
          </Transition>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import Button from '@/components/base/widget/Button.vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import ScheduleContent from './ScheduleContent.vue'
import { PawPrint } from 'lucide-vue-next'

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
