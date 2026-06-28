/**
 * 新手教程 Pinia Store
 *
 * 管理教程生命周期：启动、导航、完成、持久化。
 *
 * ## 两种模式（由后端配置决定）
 *
 * ### 全局模式 (global)
 * `SHOW_ONBOARDING_TUTORIAL=true` 且 `PER_DEVICE=false`
 * 首次访问触发教程，完成后后端将 `SHOW_ONBOARDING_TUTORIAL` 设为 false。
 * 同一用户的多个客户端/设备共享此状态。
 *
 * ### 每设备模式 (per-device)
 * `SHOW_ONBOARDING_TUTORIAL_PER_DEVICE=true`（优先级更高）
 * 每台设备独立在 localStorage 中记录完成状态。
 * 即使全局已关闭，此模式仍会为每台新设备触发教程。
 *
 * ## 交互流程
 *
 * 每个配置步骤的典型流程：
 *   提示 tooltip → 点击"下一步" → nextAction 打开面板 → isPaused 隐藏教程
 *   → 用户操作面板 → 关闭面板 → waitForField 检测到 → 推进到下一步
 *
 * 普通步骤（无 nextAction）：
 *   提示 → 点击"下一步" → 直接推进
 */
import { defineStore } from 'pinia'
import { watch } from 'vue'
import { useUIStore } from './ui/ui'
import {
  getEnvConfigByKey,
  saveEnvConfigSettings,
} from '../../api/services/config'
import { REGISTERED_PRESETS } from './tutorial-presets'
import type { TutorialAction, TutorialMode, TutorialState, TutorialStep } from '../../types/tutorial'

/** 自动推进定时器（模块级，避免被 persist 序列化） */
let autoAdvanceTimer: ReturnType<typeof setTimeout> | null = null

export const useTutorialStore = defineStore('tutorial', {
  state: (): TutorialState => ({
    isActive: false,
    isCompleted: false,
    isPaused: false,
    currentPresetId: '',
    currentStepIndex: 0,
    tutorialMode: 'idle',
    firstFrameRendered: false,
  }),

  getters: {
    /** 当前步骤定义 */
    currentStep(state): TutorialStep | null {
      if (!state.isActive || !state.currentPresetId) return null
      const preset = REGISTERED_PRESETS[state.currentPresetId]
      if (!preset) return null
      return preset.steps[state.currentStepIndex] ?? null
    },

    /** 当前预设的总步数 */
    totalSteps(state): number {
      if (!state.currentPresetId) return 0
      const preset = REGISTERED_PRESETS[state.currentPresetId]
      return preset?.steps.length ?? 0
    },

    /** 是否为最后一步 */
    isLastStep(state): boolean {
      const preset = REGISTERED_PRESETS[state.currentPresetId]
      if (!preset) return false
      return state.currentStepIndex >= preset.steps.length - 1
    },

    /** 是否为第一步 */
    isFirstStep(state): boolean {
      return state.currentStepIndex <= 0
    },
  },

  actions: {
    /**
     * 检查后端配置，确定教程模式并返回是否应该展示。
     */
    async checkBackendConfig(): Promise<boolean> {
      let perDevice = false
      let globalEnabled = false

      try {
        const perDeviceCfg = await getEnvConfigByKey('SHOW_ONBOARDING_TUTORIAL_PER_DEVICE')
        perDevice = perDeviceCfg.value === 'true'
      } catch {
        // 旧版后端没有此配置项，默认为 false
      }

      if (perDevice) {
        this.tutorialMode = 'per-device'
        return !this.isCompleted
      }

      try {
        const globalCfg = await getEnvConfigByKey('SHOW_ONBOARDING_TUTORIAL')
        globalEnabled = globalCfg.value === 'true'
      } catch {
        // 后端没有此配置项，默认不启用
      }

      if (globalEnabled) {
        this.tutorialMode = 'global'
        return true
      }

      this.tutorialMode = 'idle'
      return false
    },

    /**
     * 启动指定预设的教程
     */
    startPreset(presetId: string, stepIndex = 0) {
      const preset = REGISTERED_PRESETS[presetId]
      if (!preset) {
        console.warn(`[Tutorial] 预设 "${presetId}" 不存在`)
        return
      }

      this.currentPresetId = presetId
      this.currentStepIndex = stepIndex
      this.isActive = true
      this.isPaused = false

      // 执行当前步骤的入口 action（如果有），并设置自动推进计时器
      this._executeCurrentAction()
    },

    /**
     * 下一步（由 TutorialOverlay 在点击按钮或自动推进时调用）
     */
    nextStep() {
      const preset = REGISTERED_PRESETS[this.currentPresetId]
      if (!preset) return

      this._clearAutoAdvance()

      if (this.currentStepIndex < preset.steps.length - 1) {
        this.currentStepIndex++
        this.isPaused = false
        this._executeCurrentAction()
      } else {
        this.complete()
      }
    },

    /**
     * 上一步
     */
    prevStep() {
      if (this.currentStepIndex <= 0) return
      this._clearAutoAdvance()
      this.currentStepIndex--
      this.isPaused = false
      this._executeCurrentAction()
    },

    /**
     * 执行当前步骤的 nextAction（由 TutorialOverlay 在点击"下一步"时调用）
     * 返回 true 表示有 nextAction 需要等待
     */
    executeNextAction(): boolean {
      const step = this.currentStep
      if (!step?.nextAction) return false

      this._executeAction(step.nextAction)

      // 如果有 waitForField，暂停教程遮罩
      if (step.waitForField) {
        this.isPaused = true
        return true
      }

      return false
    },

    /**
     * 标记教程第一帧已渲染
     */
    markFirstFrameRendered() {
      this.firstFrameRendered = true
    },

    /**
     * 返回一个 Promise，在第一帧渲染完成后 resolve
     */
    waitForFirstFrame(): Promise<void> {
      if (this.firstFrameRendered) return Promise.resolve()
      return new Promise((resolve) => {
        const unwatch = watch(
          () => this.firstFrameRendered,
          (val) => {
            if (val) {
              unwatch()
              resolve()
            }
          },
        )
      })
    },

    /**
     * 跳过整个教程
     */
    skip() {
      this._clearAutoAdvance()
      this.complete()
    },

    /**
     * 标记教程完成
     */
    async complete() {
      const uiStore = useUIStore()
      uiStore.showLlmConfig = false
      if (uiStore.showSettings) {
        uiStore.toggleSettings(false)
      }

      this.isCompleted = true
      this.isActive = false
      this.isPaused = false

      if (this.tutorialMode === 'global') {
        try {
          await saveEnvConfigSettings({ SHOW_ONBOARDING_TUTORIAL: 'false' })
        } catch (e) {
          console.warn('[Tutorial] 后端关闭教程配置失败', e)
        }
      }

      this.currentPresetId = ''
      this.currentStepIndex = 0
    },

    /**
     * 重置教程状态（开发调试用）
     */
    reset() {
      this._clearAutoAdvance()
      this.isActive = false
      this.isCompleted = false
      this.isPaused = false
      this.currentPresetId = ''
      this.currentStepIndex = 0
      this.tutorialMode = 'idle'
      this.firstFrameRendered = false
    },

    // ========== 内部方法 ==========

    /** 进入步骤时执行入口 action，并设置自动推进计时器 */
    _executeCurrentAction() {
      const step = this.currentStep
      if (step?.action && step.action.type !== 'none') {
        this._executeAction(step.action)
      }

      // 设置自动推进计时器（如 welcome/complete 步骤）
      if (step?.autoAdvanceMs && step.autoAdvanceMs > 0) {
        this._scheduleAutoAdvance(step.autoAdvanceMs)
      }
    },

    /** 执行指定的 action（立即执行，无延迟） */
    _executeAction(action: TutorialAction) {
      const uiStore = useUIStore()
      const { type, payload } = action

      switch (type) {
        case 'openSettings':
          uiStore.toggleSettings(true)
          break
        case 'closeSettings':
          uiStore.toggleSettings(false)
          break
        case 'switchSettingsTab':
          if (payload?.tab) {
            uiStore.setSettingsTab(payload.tab as string)
            uiStore.toggleSettings(true)
          }
          break
        case 'openLlmConfig':
          uiStore.showLlmConfig = true
          break
      }
    },

    _scheduleAutoAdvance(ms: number) {
      this._clearAutoAdvance()
      autoAdvanceTimer = setTimeout(() => {
        autoAdvanceTimer = null
        this.nextStep()
      }, ms)
    },

    _clearAutoAdvance() {
      if (autoAdvanceTimer) {
        clearTimeout(autoAdvanceTimer)
        autoAdvanceTimer = null
      }
    },
  },

  // 启用持久化
  persist: {
    key: 'lingchat-tutorial',
    exclude: ['firstFrameRendered', 'isPaused'],
  },
})
