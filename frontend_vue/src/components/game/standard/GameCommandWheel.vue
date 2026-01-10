<template>
  <div
    v-if="isVisible"
    class="command-wheel-container"
    :style="containerStyle"
    @mousedown="startDrag"
  >
    <!-- 初始状态 -->
    <div v-if="!isExpanded" class="command-wheel-mini" @dblclick="expandWheel">
      <svg
        class="hand-icon"
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
    <div v-else class="command-wheel-expanded" @click.stop>
      <div class="command-wheel-inner">
        <!-- 分割线 -->
        <div class="divider vertical"></div>
        <div class="divider horizontal"></div>

        <!-- 触摸选项 (左上角) -->
        <div class="option-region touch-region" @click="selectCommand('touch')">
          <svg
            class="hand-icon"
            focusable="false"
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path
              d="M13 24c-3.26 0-6.19-1.99-7.4-5.02l-3.03-7.61c-.31-.79.43-1.58 1.24-1.32l.79.26c.56.18 1.02.61 1.24 1.16L7.25 15H8V3.25C8 2.56 8.56 2 9.25 2s1.25.56 1.25 1.25V12h1V1.25c0-.69.56-1.25 1.25-1.25S14 .56 14 1.25V12h1V2.75c0-.69.56-1.25 1.25-1.25s1.25.56 1.25 1.25V12h1V5.75c0-.69.56-1.25 1.25-1.25S21 5.06 21 5.75V16c0 4.42-3.58 8-8 8"
            ></path>
          </svg>

          <span class="option-label">触摸</span>
        </div>

        <div class="option-region show-region" @click="selectCommand('show')">
          <svg
            :class="['show-icon', { hide: gameStore.command == 'show' }]"
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
            :class="['show-icon', { hide: gameStore.command != 'show' }]"
            focusable="false"
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="white"
          >
            <path
              d="M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7M2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2m4.31-.78 3.15 3.15.02-.16c0-1.66-1.34-3-3-3z"
            ></path>
          </svg>
          <span class="option-label">{{ gameStore.command != 'show' ? '显示' : '隐藏' }}</span>
        </div>

        <!-- TODO: 带角色出门，切换场景 -->
        <div class="option-region"></div>

        <!-- 取消选项 (右上角) -->
        <div class="option-region cancel-region" @click="selectCommand('cancel')">
          <svg class="cancel-icon" viewBox="0 0 24 24" fill="currentColor">
            <path
              d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
            />
          </svg>
          <span class="option-label">取消</span>
        </div>
      </div>

      <!-- 点击外部关闭 -->
      <div class="command-wheel-overlay" @click="collapseWheel"></div>
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
@reference "tailwindcss";
.hide {
  @apply hidden;
}

.command-wheel-container {
  @apply fixed z-114514;
  transition: none;
}

.command-wheel-mini {
  @apply w-12 h-12 backdrop-blur-[20px] rounded-full flex items-center justify-center cursor-grab z-114514;
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  transition: all 0.3s ease;
}

.command-wheel-mini:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.05);
}

.hand-icon-mini {
  @apply w-6 h-6 text-(--accent-color);
}

.command-wheel-expanded {
  @apply relative w-48 h-48;
  animation: wheelExpand 0.5s ease-out;
}

.command-wheel-inner {
  @apply relative w-full h-full backdrop-blur-[20px] rounded-full grid grid-cols-[1fr_1fr] grid-rows-[1fr_1fr] overflow-hidden;
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
}

.divider {
  @apply absolute bg-white/30;
}

.divider.vertical {
  @apply absolute left-1/2 top-[15%] bottom-[15%] w-px bg-white/30 -translate-x-1/2;
}

.divider.horizontal {
  @apply absolute top-1/2 left-[15%] right-[15%] h-px bg-white/30 -translate-y-1/2;
}

.option-region {
  @apply flex flex-col items-center justify-center cursor-pointer rounded-full m-2.5;
  transition: background 0.3s ease;
}

.option-region:hover {
  @apply bg-white/10;
}

.touch-region {
  @apply col-start-1 row-start-1;
}

.show-region {
  @apply col-start-1 row-start-2;
}

.cancel-region {
  @apply col-start-2 row-start-1;
}

.cancel-icon {
  @apply w-8 h-8 text-[#ff6b6b] mb-1;
}

.hand-icon {
  @apply w-8 h-8 text-(--accent-color) mb-1;
}

.show-icon {
  @apply w-8 h-8 text-[#ff6b6b];
}

.option-label {
  @apply text-[12px] text-white [text-shadow:0_1px_2px_rgba(0,0,0,0.5)];
}

.command-wheel-overlay {
  @apply fixed inset-0 -z-10 cursor-default;
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
