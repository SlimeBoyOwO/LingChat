<template>
  <div
    v-if="isVisible"
    class="fixed z-[114514] transition-none"
    :style="containerStyle"
    @mousedown="startDrag"
  >
    <!-- 初始状态 -->
    <div v-if="!isExpanded" @dblclick="expandWheel" class="command-wheel-mini w-[50px] h-[50px] bg-white/20 backdrop-blur-[20px] border border-white/10 rounded-full flex items-center justify-center cursor-pointer transition-all duration-300 ease-in-out shadow-[0_4px_16px_rgba(0,0,0,0.1)] hover:bg-white/30 hover:scale-105">
      <svg
        class="w-8 h-8 text-[var(--accent-color)] mb-1"
        focusable="false"
        aria-hidden="true"
        viewBox="0 0 24 24"
        fill="currentColor"
      >
        <path
          d="M13 24c-3.26 0-6.19-1.99-7.4-5.02l-3.03-7.61c-.31-.79.43-1.58 1.24-1.32l.79.26c.56.18 1.02.61 1.24 1.16L7.25 15H8V3.25C8 2.56 8.56 2 9.25 2s1.25.56 1.25 1.25V12h1V1.25c0-.69.56-1.25 1.25-1.25S14 .56 14 1.25V12h1V2.75c0-.69.56-1.25 1.25-1.25s1.25.56 1.25 1.25V12h1V5.75c0-.69.56-1.25 1.25-1.25S21 5.06 21 5.75V16c0 4.42-3.58 8-8 8"
        ></path>
      </svg>
    </div>

    <!-- 展开状态 -->
    <div v-else class="command-wheel-expanded relative w-[200px] h-[200px]" @click.stop>
      <div class="relative w-full h-full bg-white/10 backdrop-blur-[20px] border border-white/[0.125] rounded-full shadow-[0_8px_32px_rgba(0,0,0,0.1)] grid grid-cols-2 grid-rows-2 overflow-hidden">
        <!-- 分割线 -->
        <div class="absolute left-1/2 top-[15%] bottom-[15%] w-px bg-white/30 -translate-x-1/2"></div>
        <div class="absolute top-1/2 left-[15%] right-[15%] h-px bg-white/30 -translate-y-1/2"></div>

        <!-- 触摸选项 (左上角) -->
        <div class="flex flex-col items-center justify-center cursor-pointer transition-colors duration-300 ease-in-out rounded-full m-[10px] hover:bg-white/10 col-start-1 row-start-1" @click="selectCommand('touch')">
          <svg
            class="w-8 h-8 text-[var(--accent-color)] mb-1"
            focusable="false"
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path
              d="M13 24c-3.26 0-6.19-1.99-7.4-5.02l-3.03-7.61c-.31-.79.43-1.58 1.24-1.32l.79.26c.56.18 1.02.61 1.24 1.16L7.25 15H8V3.25C8 2.56 8.56 2 9.25 2s1.25.56 1.25 1.25V12h1V1.25c0-.69.56-1.25 1.25-1.25S14 .56 14 1.25V12h1V2.75c0-.69.56-1.25 1.25-1.25s1.25.56 1.25 1.25V12h1V5.75c0-.69.56-1.25 1.25-1.25S21 5.06 21 5.75V16c0 4.42-3.58 8-8 8"
            ></path>
          </svg>

          <span class="text-[12px] text-white [text-shadow:0_1px_2px_rgba(0,0,0,0.5)]">触摸</span>
        </div>

        <div class="flex flex-col items-center justify-center cursor-pointer transition-colors duration-300 ease-in-out rounded-full m-[10px] hover:bg-white/10 col-start-1 row-start-2" @click="selectCommand('show')">
          <svg
            :class="['w-8 h-8 text-[#ff6b6b]', { hide: gameStore.command == 'show' }]"
            focusable="false"
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="white"
          >
            <path
              d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5M12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5m0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3"
            ></path>
          </svg>
          <svg
            :class="['w-8 h-8 text-[#ff6b6b]', { hide: gameStore.command != 'show' }]"
            focusable="false"
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="white"
          >
            <path
              d="M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7M2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2m4.31-.78 3.15 3.15.02-.16c0-1.66-1.34-3-3-3z"
            ></path>
          </svg>
          <span class="text-[12px] text-white [text-shadow:0_1px_2px_rgba(0,0,0,0.5)]">{{ gameStore.command != 'show' ? '显示' : '隐藏' }}</span>
        </div>

        <!-- TODO: 带角色出门，切换场景 -->
        <div class="flex flex-col items-center justify-center cursor-pointer transition-colors duration-300 ease-in-out rounded-full m-[10px] hover:bg-white/10"></div>

        <!-- 取消选项 (右上角) -->
        <div class="flex flex-col items-center justify-center cursor-pointer transition-colors duration-300 ease-in-out rounded-full m-[10px] hover:bg-white/10 col-start-2 row-start-1" @click="selectCommand('cancel')">
          <svg class="w-8 h-8 text-[#ff6b6b] mb-1" viewBox="0 0 24 24" fill="currentColor">
            <path
              d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
            />
          </svg>
          <span class="text-[12px] text-white [text-shadow:0_1px_2px_rgba(0,0,0,0.5)]">取消</span>
        </div>
      </div>

      <!-- 点击外部关闭 -->
      <div class="fixed inset-0 -z-10 cursor-default" @click="collapseWheel"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useGameStore } from '@/stores/modules/game'

interface Props {
  isVisible?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  isVisible: true,
})

const emit = defineEmits(['position-changed'])

const position = ref({ x: 0, y: 0 })
const isExpanded = ref(false)
const isDragging = ref(false)
const dragOffset = ref({ x: 0, y: 0 })

const containerStyle = computed(() => ({
  left: `${position.value.x}px`,
  top: `${position.value.y}px`,
  cursor: isDragging.value ? 'grabbing' : 'grab',
}))

const startDrag = (e: MouseEvent) => {
  if (isExpanded.value) return

  isDragging.value = true
  dragOffset.value = {
    x: e.clientX - position.value.x,
    y: e.clientY - position.value.y,
  }

  document.addEventListener('mousemove', drag)
  document.addEventListener('mouseup', stopDrag)
}

const drag = (e: MouseEvent) => {
  if (!isDragging.value) return

  position.value = {
    x: Math.max(0, Math.min(window.innerWidth - 50, e.clientX - dragOffset.value.x)),
    y: Math.max(0, Math.min(window.innerHeight - 50, e.clientY - dragOffset.value.y)),
  }

  emit('position-changed', position.value)
}

const stopDrag = () => {
  isDragging.value = false
  document.removeEventListener('mousemove', drag)
  document.removeEventListener('mouseup', stopDrag)
}

const expandWheel = () => {
  isExpanded.value = true
}

const collapseWheel = () => {
  isExpanded.value = false
}

const gameStore = useGameStore()

const selectCommand = (command: string) => {
  // TODO: 这里光标与指令icon是用MUI library的svg图标，但是MUI不提供png格式，所以只能用这种方式
  if (command === 'touch') {
    // 更改光标为手掌形状
    document.body.style.cursor = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' width='32' height='32' fill='black'%3E%3Cpath d='M13 24c-3.26 0-6.19-1.99-7.4-5.02l-3.03-7.61c-.31-.79.43-1.58 1.24-1.32l.79.26c.56.18 1.02.61 1.24 1.16L7.25 15H8V3.25C8 2.56 8.56 2 9.25 2s1.25.56 1.25 1.25V12h1V1.25c0-.69.56-1.25 1.25-1.25S14 .56 14 1.25V12h1V2.75c0-.69.56-1.25 1.25-1.25s1.25.56 1.25 1.25V12h1V5.75c0-.69.56-1.25 1.25-1.25S21 5.06 21 5.75V16c0 4.42-3.58 8-8 8'/%3E%3C/svg%3E") 0 0, auto`
    gameStore.command = command
  } else if (command == 'show') {
    document.body.style.cursor = 'default'
    // 显示或隐藏可交互区域
    gameStore.command = gameStore.command == 'show' ? 'unshow' : 'show'
  } else if (command === 'cancel') {
    // 恢复光标为默认样式
    document.body.style.cursor = 'default'
    gameStore.command = null
  }

  collapseWheel()
}

// 初始化位置到右下角
onMounted(() => {
  position.value = {
    x: window.innerWidth - 200,
    y: window.innerHeight / 2,
  }
})

onUnmounted(() => {
  document.removeEventListener('mousemove', drag)
  document.removeEventListener('mouseup', stopDrag)
})

const resetPosition = () => {
  position.value = {
    x: window.innerWidth - 200,
    y: window.innerHeight / 2,
  }
}

defineExpose({
  resetPosition,
})
</script>

<style scoped>
.hide {
  display: none;
}

.command-wheel-mini {
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
}

.command-wheel-mini:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.05);
}

.command-wheel-expanded {
  animation: wheelExpand 0.5s ease-out;
}

@keyframes wheelExpand {
  0% {
    opacity: 0;
    transform: scale(0) rotate(180deg);
  }
  100% {
    opacity: 1;
    transform: scale(1) rotate(0deg);
  }
}
</style>