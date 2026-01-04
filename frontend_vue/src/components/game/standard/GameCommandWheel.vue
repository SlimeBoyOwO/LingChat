<template>
  <div
    v-if="isVisible"
    class="command-wheel-container"
    :style="containerStyle"
    @mousedown="startDrag"
  >
    <!-- 小圆形状态 -->
    <div v-if="!isExpanded" class="command-wheel-mini" @dblclick="expandWheel">
      <svg class="hand-icon-mini" viewBox="0 0 24 24" fill="currentColor">
        <path d="M18 9.5V7a2 2 0 0 0-4 0v2.5l-2 0V7a4 4 0 0 0-8 0v6.5a6.5 6.5 0 0 0 6.5 6.5h.5a6.5 6.5 0 0 0 6.5-6.5V7a2 2 0 0 0-4 0v2.5l-2 0z"/>
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
          <svg class="hand-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M18 9.5V7a2 2 0 0 0-4 0v2.5l-2 0V7a4 4 0 0 0-8 0v6.5a6.5 6.5 0 0 0 6.5 6.5h.5a6.5 6.5 0 0 0 6.5-6.5V7a2 2 0 0 0-4 0v2.5l-2 0z"/>
          </svg>
          <span class="option-label">触摸</span>
        </div>

        <!-- 其他选项区域 (占位符) -->
        <div class="option-region"></div>
        <div class="option-region"></div>
        <div class="option-region"></div>
      </div>

      <!-- 点击外部关闭 -->
      <div class="command-wheel-overlay" @click="collapseWheel"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

interface Props {
  isVisible?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  isVisible: true
})

const emit = defineEmits(['command-selected', 'position-changed'])

const position = ref({ x: 0, y: 0 })
const isExpanded = ref(false)
const isDragging = ref(false)
const dragOffset = ref({ x: 0, y: 0 })

const containerStyle = computed(() => ({
  left: `${position.value.x}px`,
  top: `${position.value.y}px`,
  cursor: isDragging.value ? 'grabbing' : 'grab'
}))

const startDrag = (e: MouseEvent) => {
  if (isExpanded.value) return

  isDragging.value = true
  dragOffset.value = {
    x: e.clientX - position.value.x,
    y: e.clientY - position.value.y
  }

  document.addEventListener('mousemove', drag)
  document.addEventListener('mouseup', stopDrag)
}

const drag = (e: MouseEvent) => {
  if (!isDragging.value) return

  position.value = {
    x: Math.max(0, Math.min(window.innerWidth - 50, e.clientX - dragOffset.value.x)),
    y: Math.max(0, Math.min(window.innerHeight - 50, e.clientY - dragOffset.value.y))
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

const selectCommand = (command: string) => {
  if (command === 'touch') {
    // 更改光标为手掌形状
    document.body.style.cursor = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='%23000'%3E%3Cpath d='M18 9.5V7a2 2 0 0 0-4 0v2.5l-2 0V7a4 4 0 0 0-8 0v6.5a6.5 6.5 0 0 0 6.5 6.5h.5a6.5 6.5 0 0 0 6.5-6.5V7a2 2 0 0 0-4 0v2.5l-2 0z'/%3E%3C/svg%3E"), auto`
  }

  emit('command-selected', command)
  collapseWheel()
}

// 初始化位置到右下角
onMounted(() => {
  position.value = {
    x: window.innerWidth - 200,
    y: window.innerHeight / 2
  }
})

onUnmounted(() => {
  document.removeEventListener('mousemove', drag)
  document.removeEventListener('mouseup', stopDrag)
})
</script>

<style scoped>
.command-wheel-container {
  position: fixed;
  z-index: 114514;
  transition: none;
}

.command-wheel-mini {
  width: 50px;
  height: 50px;
  background: rgba(255, 255, 255, 0.2);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.125);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.3s ease;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
}

.command-wheel-mini:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.05);
}

.hand-icon-mini {
  width: 24px;
  height: 24px;
  color: var(--accent-color);
}

.command-wheel-expanded {
  position: relative;
  width: 300px;
  height: 300px;
  animation: wheelExpand 0.5s ease-out;
}

.command-wheel-inner {
  position: relative;
  width: 100%;
  height: 100%;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.125);
  border-radius: 50%;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
}

.divider {
  position: absolute;
  background: rgba(255, 255, 255, 0.3);
}

.divider.vertical {
  left: 50%;
  top: 15%;
  bottom: 15%;
  width: 1px;
  transform: translateX(-50%);
}

.divider.horizontal {
  top: 50%;
  left: 15%;
  right: 15%;
  height: 1px;
  transform: translateY(-50%);
}

.option-region {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.3s ease;
  border-radius: 50%;
  margin: 10px;
}

.option-region:hover {
  background: rgba(255, 255, 255, 0.1);
}

.touch-region {
  grid-column: 1;
  grid-row: 1;
}

.hand-icon {
  width: 32px;
  height: 32px;
  color: var(--accent-color);
  margin-bottom: 4px;
}

.option-label {
  font-size: 12px;
  color: white;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
}

.command-wheel-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: -1;
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
