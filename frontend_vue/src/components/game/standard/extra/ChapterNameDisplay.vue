<template>
  <div class="w-full pt-8">
    <transition
      @before-enter="beforeEnter"
      @enter="enter"
      @after-enter="afterEnter"
      @before-leave="beforeLeave"
      @leave="leave"
      @after-leave="afterLeave"
    >
      <div
        v-if="shouldShowChapterName"
        class="relative drop-shadow-[0_8px_12px_rgba(0,0,0,0.25)] w-[20%]"
      >
        <div
          class="relative flex items-center pl-8 pr-14 py-3 bg-slate-900/40 backdrop-blur-md overflow-hidden"
          style="
            clip-path: polygon(
              0 0,
              calc(100% - 24px) 0,
              100% 50%,
              calc(100% - 24px) 100%,
              0 100%
            );
          "
        >
          <div class="absolute left-0 top-0 bottom-0 w-1.5 bg-brand rounded-2xl"></div>
          <Columns2 class="text-white mr-4" />
          <div
            class="text-xl font-bold text-gray-300 tracking-widest drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]"
          >
            {{ displayChapterName }}
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { Columns2 } from 'lucide-vue-next'

const gameStore = useGameStore()
const uiStore = useUIStore()

const displayChapterName = ref('')
const showChapterName = ref(false)
const isTransitioning = ref(false)

const shouldShowChapterName = computed(() => {
  return uiStore.showSettings ? false : showChapterName.value
})

// 监听章节名称变化
watch(
  () => gameStore.runningScript?.currentChapterName,
  async (newName, oldName) => {
    if (isTransitioning.value) return
    if (newName === oldName) return

    if (!oldName && newName) {
      displayChapterName.value = newName
      showChapterName.value = true
    } else if (oldName && !newName) {
      showChapterName.value = false
    } else if (oldName && newName && oldName !== newName) {
      isTransitioning.value = true
      showChapterName.value = false

      setTimeout(() => {
        displayChapterName.value = newName
        showChapterName.value = true
        isTransitioning.value = false
      }, 500)
    }
  },
)

// 动画函数
function beforeEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'translateX(-20px)'
  element.style.transition = 'all 0.5s cubic-bezier(0.4, 0, 0.2, 1)'
}

function enter(el: Element, done: () => void) {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const element = el as HTMLElement
      element.style.opacity = '1'
      element.style.transform = 'translateX(0)'
      setTimeout(done, 500)
    })
  })
}

function afterEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '1'
  element.style.transform = 'translateX(0)'
}

function beforeLeave(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '1'
  element.style.transform = 'translateX(0)'
  element.style.transition = 'all 0.5s cubic-bezier(0.4, 0, 0.2, 1)'
}

function leave(el: Element, done: () => void) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'translateX(20px)'
  setTimeout(done, 500)
}

function afterLeave(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
}
</script>
