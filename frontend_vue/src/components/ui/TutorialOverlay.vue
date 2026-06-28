<template>
  <Teleport to="body">
    <Transition name="tutorial-fade">
      <div
        v-if="tutorialStore.isActive && !tutorialStore.isPaused && currentStep"
        class="tutorial-overlay"
        :class="{ 'tutorial-overlay--center': isCenterMode }"
      >
        <!-- ===== SVG Spotlight 遮罩（仅非 center 模式） ===== -->
        <svg
          v-if="!isCenterMode && spotlightRect"
          class="tutorial-spotlight-svg"
          aria-hidden="true"
        >
          <defs>
            <mask :id="maskId">
              <!-- 全屏白色（可见区域） -->
              <rect width="100%" height="100%" fill="white" />
              <!-- 目标区域黑色（挖洞透明） -->
              <rect
                :x="spotlightRect.x"
                :y="spotlightRect.y"
                :width="spotlightRect.width"
                :height="spotlightRect.height"
                fill="black"
                :rx="12"
              />
            </mask>
          </defs>
          <!-- 半透明遮罩层 -->
          <rect width="100%" height="100%" fill="rgba(0, 0, 0, 0.6)" :mask="`url(#${maskId})`" />
        </svg>

        <!-- ===== 背景遮罩（center 模式） ===== -->
        <div v-else class="tutorial-backdrop"></div>

        <!-- ===== Tooltip 浮层（role="dialog" for a11y） ===== -->
        <div
          ref="tooltipRef"
          class="tutorial-tooltip"
          role="dialog"
          :aria-label="currentStep.title"
          :class="[
            isCenterMode
              ? 'tutorial-tooltip--center'
              : `tutorial-tooltip--${currentStep.tooltipPlacement || 'bottom'}`,
          ]"
          :style="tooltipStyle"
          tabindex="-1"
        >
          <!-- 步骤计数器 -->
          <div class="tutorial-step-counter">
            {{ tutorialStore.currentStepIndex + 1 }} / {{ tutorialStore.totalSteps }}
          </div>

          <!-- 标题 -->
          <h3 class="tutorial-title">{{ currentStep.title }}</h3>

          <!-- 内容 -->
          <p class="tutorial-content">{{ currentStep.content }}</p>

          <!-- 进度条 -->
          <div class="tutorial-progress-bar">
            <div
              class="tutorial-progress-fill"
              :style="{
                width: `${((tutorialStore.currentStepIndex + 1) / tutorialStore.totalSteps) * 100}%`,
              }"
            ></div>
          </div>

          <!-- 按钮区域 -->
          <div class="tutorial-actions">
            <button
              v-if="canGoBack"
              class="tutorial-btn tutorial-btn--back"
              @click="tutorialStore.prevStep()"
            >
              上一步
            </button>

            <button
              v-if="canSkip"
              class="tutorial-btn tutorial-btn--skip"
              @click="tutorialStore.skip()"
            >
              跳过
            </button>

            <button
              ref="nextBtnRef"
              class="tutorial-btn tutorial-btn--next"
              :disabled="isHandlingNext"
              @click="handleNext"
            >
              {{ tutorialStore.isLastStep ? '完成' : '下一步' }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useTutorialStore } from '../../stores/modules/tutorial'
import { useUIStore } from '../../stores/modules/ui/ui'
import type { TutorialStep } from '../../types/tutorial'

const tutorialStore = useTutorialStore()
const uiStore = useUIStore()

/** 当前步骤（快捷引用） */
const currentStep = computed<TutorialStep | null>(() => tutorialStore.currentStep)

/** 是否为中心提示模式 */
const isCenterMode = computed(() => {
  const step = currentStep.value
  return !step || step.tooltipPlacement === 'center' || !step.highlightSelector
})

/** 是否可以返回上一步 */
const canGoBack = computed(() => {
  const step = currentStep.value
  return step?.allowBack !== false && !tutorialStore.isFirstStep
})

/** 是否可以跳过 */
const canSkip = computed(() => {
  const step = currentStep.value
  return step?.skippable !== false
})

/** 正在处理 nextAction（防止重复点击） */
const isHandlingNext = ref(false)

/** waitForField 到 store 字段的映射（类型安全） */
const WAIT_FIELD_MAP: Record<string, keyof typeof uiStore> = {
  showLlmConfig: 'showLlmConfig',
  showSettings: 'showSettings',
}

/**
 * 点击"下一步"的处理函数
 *
 * 流程：
 * 1. 如果当前步骤有 nextAction → 执行 → 暂停教程遮罩 → 等待 waitForField → 推进
 * 2. 如果没有 nextAction → 直接推进到下一步
 */
async function handleNext() {
  if (isHandlingNext.value) return
  const step = currentStep.value
  if (!step) return

  // 检查是否有 nextAction（点击下一步时触发的操作）
  const hasNextAction = !!step.nextAction

  if (hasNextAction) {
    isHandlingNext.value = true
    // 执行 nextAction（例如打开 LLM 面板或设置面板）
    tutorialStore.executeNextAction()

    // 如果有 waitForField，等待面板关闭后自动推进
    if (step.waitForField) {
      const fieldKey = WAIT_FIELD_MAP[step.waitForField]
      if (fieldKey) {
        await new Promise<void>((resolve) => {
          const unwatch = watch(
            () => uiStore[fieldKey],
            (val) => {
              if (!val) {
                unwatch()
                resolve()
              }
            },
          )
          // 超时保护：2 分钟后无论是否关闭都继续
          const timeoutId = setTimeout(() => {
            unwatch()
            resolve()
          }, 120000)
        })
      }
    }

    isHandlingNext.value = false
  }

  // 推进到下一步
  tutorialStore.nextStep()
}

// ========== 键盘事件（Escape 关闭） ==========

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && tutorialStore.isActive && !tutorialStore.isPaused) {
    tutorialStore.skip()
  }
}

// ========== 焦点管理 ==========

const tooltipRef = ref<HTMLElement | null>(null)
const nextBtnRef = ref<HTMLElement | null>(null)

/** 步骤变化后移动焦点到 tooltip */
function focusTooltip() {
  nextTick(() => {
    tooltipRef.value?.focus({ preventScroll: true })
  })
}

// ========== SVG mask 唯一 ID（防止多实例冲突） ==========

const maskId = `tutorial-spotlight-mask-${Date.now()}`

// ========== Spotlight 定位逻辑 ==========

interface SpotlightRect {
  x: number
  y: number
  width: number
  height: number
}

const spotlightRect = ref<SpotlightRect | null>(null)
const tooltipStyle = ref<Record<string, string>>({})

/** 滚动使高亮目标可见 */
function scrollTargetIntoView(selector: string) {
  const target = document.querySelector(selector)
  if (target) {
    target.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }
}

/** 计算目标元素位置并更新 spotlight 和 tooltip 位置 */
function updateSpotlight() {
  const step = currentStep.value
  if (!step || !step.highlightSelector) {
    spotlightRect.value = null
    tooltipStyle.value = {}
    return
  }

  const target = document.querySelector(step.highlightSelector)
  if (!target) {
    spotlightRect.value = null
    tooltipStyle.value = {}
    return
  }

  const rect = target.getBoundingClientRect()
  const padding = step.spotlightPadding ?? 8

  spotlightRect.value = {
    x: rect.left - padding,
    y: rect.top - padding,
    width: rect.width + padding * 2,
    height: rect.height + padding * 2,
  }

  // 计算 tooltip 位置（非 center 模式）
  const placement = step.tooltipPlacement || 'bottom'
  const gap = 16

  switch (placement) {
    case 'top':
      tooltipStyle.value = {
        left: `${rect.left + rect.width / 2}px`,
        top: `${rect.top - gap}px`,
        transform: 'translate(-50%, -100%)',
      }
      break
    case 'bottom':
      tooltipStyle.value = {
        left: `${rect.left + rect.width / 2}px`,
        top: `${rect.bottom + gap}px`,
      }
      break
    case 'left':
      tooltipStyle.value = {
        left: `${rect.left - gap}px`,
        top: `${rect.top + rect.height / 2}px`,
        transform: 'translate(-100%, -50%)',
      }
      break
    case 'right':
      tooltipStyle.value = {
        left: `${rect.right + gap}px`,
        top: `${rect.top + rect.height / 2}px`,
        transform: 'translateY(-50%)',
      }
      break
    default:
      tooltipStyle.value = {
        left: `${rect.left + rect.width / 2}px`,
        top: `${rect.bottom + gap}px`,
      }
  }
}

/**
 * 第一帧已渲染标记（用于 Loader 关闭协调）
 * 当教程 overlay 首次变为可见时，通知 store
 */
let firstFrameSignalled = false
watch(
  () => tutorialStore.isActive && !!currentStep.value,
  (visible) => {
    if (visible && !firstFrameSignalled) {
      firstFrameSignalled = true
      nextTick(() => {
        tutorialStore.markFirstFrameRendered()
      })
    }
  },
  { immediate: true },
)

/** 监视步骤变化（索引或 preset 变化），重新定位并聚焦 */
watch(
  () => [tutorialStore.currentStepIndex, tutorialStore.currentPresetId],
  async () => {
    await nextTick()
    // 如果当前步骤有高亮目标，尝试滚入视野
    const step = currentStep.value
    if (step?.highlightSelector) {
      scrollTargetIntoView(step.highlightSelector)
      // 等待滚动完成后再定位 spotlight
      await nextTick()
    }
    updateSpotlight()
    focusTooltip()
  },
)

// 监听窗口变化
function handleResize() {
  updateSpotlight()
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
  window.addEventListener('resize', handleResize)
  window.addEventListener('scroll', handleResize, true)
  // 初始定位
  nextTick(() => {
    updateSpotlight()
    focusTooltip()
  })
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('resize', handleResize)
  window.removeEventListener('scroll', handleResize, true)
})
</script>

<style scoped>
/* ===== 覆盖层容器 ===== */
.tutorial-overlay {
  position: fixed;
  inset: 0;
  z-index: 10001;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  pointer-events: none; /* 允许点击穿透到被高亮元素 */
}

.tutorial-overlay--center {
  align-items: center;
}

/* ===== SVG 遮罩 ===== */
.tutorial-spotlight-svg {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

/* ===== 纯背景遮罩（center 模式） ===== */
.tutorial-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  pointer-events: none;
}

/* ===== Tooltip 浮层 ===== */
.tutorial-tooltip {
  position: absolute;
  pointer-events: auto; /* tooltip 本身可以交互 */
  max-width: 400px;
  min-width: 280px;
  padding: 24px;

  /* 玻璃态效果（与 Notification 一致） */
  background: linear-gradient(135deg, rgba(30, 30, 40, 0.97) 0%, rgba(20, 20, 30, 0.95) 100%);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-radius: 16px;
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.5),
    inset 0 1px 0 rgba(255, 255, 255, 0.1),
    0 0 20px rgba(121, 217, 255, 0.15);
}

/* 聚焦轮廓（键盘用户可见） */
.tutorial-tooltip:focus {
  outline: 2px solid rgba(121, 217, 255, 0.5);
  outline-offset: 4px;
}

.tutorial-tooltip--center {
  position: relative;
  transform: none;
}

/* ===== 步骤计数器 ===== */
.tutorial-step-counter {
  font-size: 11px;
  font-weight: 600;
  color: rgba(121, 217, 255, 0.8);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
}

/* ===== 标题 ===== */
.tutorial-title {
  font-size: 18px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.95);
  margin: 0 0 12px 0;
  line-height: 1.3;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

/* ===== 内容 ===== */
.tutorial-content {
  font-size: 14px;
  line-height: 1.7;
  color: rgba(255, 255, 255, 0.8);
  margin: 0 0 16px 0;
  white-space: pre-line;
}

/* ===== 进度条 ===== */
.tutorial-progress-bar {
  height: 3px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  margin-bottom: 16px;
  overflow: hidden;
}

.tutorial-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, rgba(121, 217, 255, 0.8), rgba(121, 217, 255, 0.4));
  border-radius: 2px;
  transition: width 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

/* ===== 按钮区域 ===== */
.tutorial-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.tutorial-btn {
  padding: 8px 18px;
  border: none;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition:
    background 0.2s,
    transform 0.15s,
    opacity 0.2s;
}

.tutorial-btn:active {
  transform: scale(0.96);
}

/* 下一步 / 完成按钮 */
.tutorial-btn--next {
  background: rgba(121, 217, 255, 0.9);
  color: #0a0a14;
  margin-left: auto;
}

.tutorial-btn--next:hover {
  background: rgba(121, 217, 255, 1);
}

/* 上一步按钮 */
.tutorial-btn--back {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.7);
}

.tutorial-btn--back:hover {
  background: rgba(255, 255, 255, 0.15);
  color: rgba(255, 255, 255, 0.9);
}

/* 跳过按钮 */
.tutorial-btn--skip {
  background: transparent;
  color: rgba(255, 255, 255, 0.4);
  font-weight: 400;
}

.tutorial-btn--skip:hover {
  color: rgba(255, 255, 255, 0.7);
}

/* ===== 进入/离开动画 ===== */
.tutorial-fade-enter-active {
  transition: opacity 0.3s ease;
}

.tutorial-fade-leave-active {
  transition: opacity 0.2s ease;
}

.tutorial-fade-enter-from,
.tutorial-fade-leave-to {
  opacity: 0;
}
</style>
