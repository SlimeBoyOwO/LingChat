<template>
  <div
    v-if="shouldShow"
    ref="badgeRef"
    class="mood-badge"
    :class="{ dragging: isDragging }"
    :style="badgeStyle"
    @pointerdown.prevent="onPointerDown"
  >
    <div class="mood-row">
      <span class="mood-label">心情</span>
      <span class="mood-value">{{ currentMood }}</span>
    </div>
    <div v-if="currentMotion" class="mood-row">
      <span class="mood-label">动作</span>
      <span class="motion-value">{{ currentMotion }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'

const STORAGE_KEY = 'lingchat_role_mood_badge_pos'

interface Position {
  x: number
  y: number
}

const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

const badgeRef = ref<HTMLElement | null>(null)

const isDragging = ref(false)
const position = ref<Position>({ x: 40, y: 96 })
const dragOffset = ref({ x: 0, y: 0 })
const transitionEnabled = ref(true)

const loadPosition = () => {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) {
      const parsed = JSON.parse(saved) as Position
      position.value = parsed
    }
  } catch {
    /* ignore */
  }
}

const savePosition = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(position.value))
  } catch {
    /* ignore */
  }
}

const badgeStyle = computed(() => {
  const pos = position.value
  return {
    position: 'fixed' as const,
    left: `${pos.x}px`,
    bottom: `${pos.y}px`,
    zIndex: 2000,
    touchAction: 'none' as const,
    transition: transitionEnabled.value ? 'all 0.3s cubic-bezier(0.22, 0.8, 0.22, 1)' : 'none',
  }
})

const shouldShow = computed(() => {
  if (!settingsStore.codeMode) return false
  if (gameStore.runningScript?.isRunning) return false
  return gameStore.mainRoleId > 0
})

const currentMood = computed(() => {
  const role = gameStore.gameRoles[gameStore.mainRoleId]
  if (!role) return '正常'
  return role.originalEmotion || role.emotion || '正常'
})

const currentMotion = computed(() => {
  const history = gameStore.dialogHistory
  for (let i = history.length - 1; i >= 0; i--) {
    const msg = history[i]
    if (msg && msg.type === 'reply' && msg.motionText) {
      return msg.motionText
    }
  }
  return uiStore.showCharacterLine || ''
})

// ====== Pointer Events（统一处理鼠标 + 触屏） ======
//
// 最佳实践参考: https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events/Using_Pointer_Events
//
// 关键要点:
//   1. touch-action: none —— 阻止浏览器默认触摸行为（滚动/缩放）
//   2. setPointerCapture() —— 指针移出元素后仍接收事件
//   3. preventDefault() —— 阻止同源的 mouse 事件被额外派发
//   4. isPrimary 检查 —— 只响应主指针（鼠标左键 / 主要触点）

const onPointerDown = (e: PointerEvent) => {
  if (!badgeRef.value) return
  if (!e.isPrimary) return // 忽略非主指针（多指触控的额外触点）

  isDragging.value = true
  transitionEnabled.value = false

  // 捕获指针：后续 pointermove/pointerup/pointercancel 都派发给 badgeRef
  // 即使手指/鼠标移出元素边界也能持续跟踪
  badgeRef.value.setPointerCapture(e.pointerId)

  const rect = badgeRef.value.getBoundingClientRect()
  dragOffset.value = {
    x: e.clientX - rect.left,
    y: rect.bottom - e.clientY, // 相对于底部（bottom）的偏移
  }

  // 指针捕获后，事件一定到达 badgeRef，但监听 document 更保险
  // （避免捕获异常丢失时事件无人处理）
  document.addEventListener('pointermove', onPointerMove)
  document.addEventListener('pointerup', onPointerUp)
  document.addEventListener('pointercancel', onPointerUp)
}

const onPointerMove = (e: PointerEvent) => {
  if (!isDragging.value) return
  // preventDefault() 阻止浏览器派发同源 mouse 事件（Pointer Events 规范要求）
  e.preventDefault()

  const vw = window.innerWidth
  const vh = window.innerHeight
  const badgeW = badgeRef.value?.offsetWidth ?? 120
  const badgeH = badgeRef.value?.offsetHeight ?? 60

  position.value = {
    x: Math.max(8, Math.min(vw - badgeW - 8, e.clientX - dragOffset.value.x)),
    y: Math.max(8, Math.min(vh - badgeH - 8, vh - e.clientY - dragOffset.value.y)),
  }
}

const cleanupPointerCapture = () => {
  if (badgeRef.value) {
    // 主动释放指针捕获（浏览器在 pointerup/pointercancel 时也会自动释放，
    // 此处显式调用确保一致行为）
    badgeRef.value.releasePointerCapture(0) // pointerId 无需精确匹配
  }
}

const onPointerUp = () => {
  if (isDragging.value) {
    isDragging.value = false
    transitionEnabled.value = true
    savePosition()
  }
  cleanupPointerCapture()
  document.removeEventListener('pointermove', onPointerMove)
  document.removeEventListener('pointerup', onPointerUp)
  document.removeEventListener('pointercancel', onPointerUp)
}

onMounted(() => {
  loadPosition()
})

onUnmounted(() => {
  cleanupPointerCapture()
  document.removeEventListener('pointermove', onPointerMove)
  document.removeEventListener('pointerup', onPointerUp)
  document.removeEventListener('pointercancel', onPointerUp)
})
</script>

<style scoped>
.mood-badge {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 12px;
  border-radius: 12px;
  background: rgba(15, 23, 42, 0.55);
  border: 1px solid rgba(125, 211, 252, 0.3);
  box-shadow:
    0 12px 34px rgba(0, 0, 0, 0.35),
    inset 0 1px 1px rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(24px) saturate(175%);
  -webkit-backdrop-filter: blur(24px) saturate(175%);
  color: #fff;
  font-size: 12px;
  line-height: 1.5;
  cursor: grab;
  user-select: none;
  min-width: 100px;
}

.mood-badge.dragging {
  cursor: grabbing;
}

.mood-badge:hover {
  background: rgba(15, 23, 42, 0.7);
  box-shadow:
    0 16px 40px rgba(0, 0, 0, 0.4),
    inset 0 1px 1px rgba(255, 255, 255, 0.2);
}

.mood-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mood-label {
  color: rgba(186, 230, 253, 0.62);
  font-size: 11px;
  font-weight: bold;
  width: 28px;
  text-align: justify;
  text-align-last: justify;
  flex-shrink: 0;
}

.mood-value {
  color: rgb(186, 230, 253);
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 120px;
}

.motion-value {
  color: rgba(248, 250, 252, 0.92);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 120px;
}
</style>
