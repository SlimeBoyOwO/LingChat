/**
 * 对话框外观管理 composable
 * 封装对话框的视觉样式计算和交互行为（滚轮历史、空格隐藏、思考时自动隐藏）
 */
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useSettingsStore } from '@/stores/modules/settings'
import { useGameStore } from '@/stores/modules/game'
import { hexToRgba } from '@/utils/color'

export interface UseDialogAppearanceOptions {
  /** 打开历史记录面板的回调 */
  openHistory: () => void
}

export function useDialogAppearance(options: UseDialogAppearanceOptions) {
  const settingsStore = useSettingsStore()
  const gameStore = useGameStore()

  // ── 外观配置（响应式读取 settings store） ──
  const dialogBgImage = computed(() => settingsStore.dialogBackgroundImage)
  const dialogOpacity = computed(() => settingsStore.dialogOpacity)
  const dialogBlur = computed(() => settingsStore.dialogBlur)
  const dialogBorderRadius = computed(() => settingsStore.dialogBorderRadius)
  const dialogGradientColor = computed(() => settingsStore.dialogGradientColor)
  const dialogTextColorValue = computed(() => settingsStore.dialogTextColor)

  // ── 交互行为开关（用 computed 追踪 getter 的变化） ──
  const scrollHistoryEnabled = computed(() => settingsStore.dialogScrollHistoryEnabled)
  const spacebarHideEnabled = computed(() => settingsStore.dialogSpacebarHideEnabled)
  const autoHideOnThinkEnabled = computed(() => settingsStore.dialogAutoHideOnThinkEnabled)

  // ── 对话框隐藏状态 ──
  const isHidden = ref(false)

  /** 隐藏对话框 */
  function hide() {
    isHidden.value = true
  }

  // ── 样式计算 ──
  const dialogWrapperStyle = computed(() => {
    const hasImage = Boolean(dialogBgImage.value)
    const style: Record<string, string> = {
      color: dialogTextColorValue.value,
    }

    if (hasImage) {
      // 使用用户自定义图片
      style.backgroundImage = `url(${dialogBgImage.value})`
      style.backgroundSize = 'cover'
      style.backgroundPosition = 'center'
      style.backdropFilter = `blur(${dialogBlur.value}px)`
      style.backgroundColor = 'rgba(0,0,0,0.2)'
    } else {
      // 使用纯渐变色
      style.background = `linear-gradient(to top, ${hexToRgba(dialogGradientColor.value, dialogOpacity.value)}, ${hexToRgba(dialogGradientColor.value, Math.max(0, dialogOpacity.value - 0.1))})`
      style.backdropFilter = 'none'
    }

    return style
  })

  // ── 滚轮查看历史记录 ──
  function handleWheelHistory(e: WheelEvent) {
    if (!scrollHistoryEnabled.value) return
    // 向上滚动 (deltaY < 0) 打开历史面板
    if (e.deltaY < -10) {
      options.openHistory()
    }
  }

  // ── 空格键隐藏/显示对话框 ──
  function handleKeydown(e: KeyboardEvent) {
    if (!spacebarHideEnabled.value) return
    // 在输入框中不触发
    const target = e.target as HTMLElement
    if (
      target &&
      (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)
    ) {
      return
    }
    if (e.code === 'Space') {
      e.preventDefault()
      isHidden.value = !isHidden.value
    }
  }

  // ── 生命周期：绑定键盘事件 ──
  onMounted(() => {
    document.addEventListener('keydown', handleKeydown)
  })

  onUnmounted(() => {
    document.removeEventListener('keydown', handleKeydown)
  })

  return {
    isHidden,
    hide,
    dialogWrapperStyle,
    dialogTextColorValue,
    handleWheelHistory,
  }
}
