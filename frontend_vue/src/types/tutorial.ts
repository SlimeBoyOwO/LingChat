/**
 * 新手教程类型定义
 *
 * 教程系统通过复用已有组件和 Store 来引导用户操作。
 * 教程是"导演"，已有组件是"演员"——通过调用 Store 的公开方法驱动界面，
 * 配合 spotlight 高亮和 tooltip 提示来引导用户。
 */

/** 工具提示位置 */
export type TooltipPlacement = 'top' | 'bottom' | 'left' | 'right' | 'center'

/**
 * 步骤激活时执行的动作——全部通过已有 Store 方法实现。
 * 将 UI 操作抽象为可序列化的 action，使教程预设可以声明式驱动界面。
 */
export interface TutorialAction {
  type: 'openSettings' | 'closeSettings' | 'switchSettingsTab' | 'openLlmConfig' | 'none'
  payload?: Record<string, unknown>
}

/** 单步定义 */
export interface TutorialStep {
  id: string
  title: string
  content: string // 支持纯文本
  highlightSelector?: string // CSS 选择器，用于 spotlight 高亮
  spotlightPadding?: number // 高亮区域 padding（默认 8px）
  tooltipPlacement: TooltipPlacement
  action?: TutorialAction // 进入步骤时执行的动作
  /** 点击"下一步"时执行的动作（触发后会隐藏教程，等待操作完成） */
  nextAction?: TutorialAction
  /** 与 nextAction 配合：隐藏教程后等待此 store 字段变为 false 才继续 */
  waitForField?: 'showLlmConfig' | 'showSettings'
  skippable?: boolean // 默认 true
  allowBack?: boolean // 默认 true
  autoAdvanceMs?: number // 0 = 手动推进
}

/** 教程预设（一组步骤的集合） */
export interface TutorialPreset {
  id: string
  name: string
  steps: TutorialStep[]
}

/** 教程模式 */
export type TutorialMode = 'idle' | 'global' | 'per-device'

/** 教程 Store 状态 */
export interface TutorialState {
  isActive: boolean
  isCompleted: boolean
  /** 教程遮罩暂时隐藏（等待用户完成配置面板操作） */
  isPaused: boolean
  currentPresetId: string
  currentStepIndex: number
  /** 当前教程模式（由后端配置决定） */
  tutorialMode: TutorialMode
  /** 第一帧是否已渲染（用于协调 Loader 关闭时机） */
  firstFrameRendered: boolean
}
