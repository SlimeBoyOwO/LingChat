import { invoke } from '@tauri-apps/api/core'

export type EmotionModelType = '9d' | 'onnx' | 'disabled'

export interface EmotionModelInfo {
  current: EmotionModelType
  available: EmotionModelType[]
}

/**
 * Get the current emotion model type setting.
 */
export async function getEmotionModelType(): Promise<EmotionModelType> {
  try {
    const item = await invoke<any>('get_setting_by_key', {
      key: 'features.emotion_model_type',
    })
    const value = (item?.value as string) ?? '9d'
    if (value === 'onnx') return 'onnx'
    if (value === 'disabled') return 'disabled'
    return '9d'
  } catch {
    return '9d'
  }
}

/**
 * Save the emotion model type setting.
 */
export async function setEmotionModelType(
  modelType: EmotionModelType,
): Promise<void> {
  await invoke('save_settings', {
    values: { 'features.emotion_model_type': modelType },
  })
}

/**
 * Hot-swap the emotion model without restart: persists the setting,
 * rebuilds the classifier, and swaps it into the running chat processor.
 * Returns a human-readable status string.
 */
export async function setEmotionModelLive(
  modelType: EmotionModelType,
): Promise<string> {
  return invoke<string>('set_emotion_model', { modelType })
}

export const EMOTION_MODEL_LABELS: Record<
  EmotionModelType,
  { label: string; description: string }
> = {
  '9d': {
    label: '九维情绪模型',
    description: '基于 Rust 的 9 维度情绪分析，更精细的情感表达',
  },
  onnx: {
    label: '经典 ONNX 模型',
    description: '基于 BERT 的 19 分类情绪模型，兼容旧版',
  },
  disabled: {
    label: '关闭情绪分类',
    description: '不进行情绪分类，角色按默认状态表现',
  },
}

// ─────────────────────────────────────────────────────────────
// 有状态的情绪心智引擎（9D Model，跨调用累积工作/长期记忆）
// ─────────────────────────────────────────────────────────────

/** 一次情绪事件（5 维刺激）输入 */
export interface EmotionEventInput {
  goalGain: number
  environmentalRisk: number
  socialInteraction: number
  expectationDeviation: number
  boundaryViolation: number
}

/** 情绪引擎当前状态（含记忆分层统计） */
export interface EmotionState {
  tick: number
  nt: number[]
  u: number[]
  v: number[]
  active: number
  dormant: number
  collapsed: number
  longTerm: number
  summary: string
}

/** 一次事件后的行为输出摘要 */
export interface EmotionBehavior {
  description: string
  expression: number[]
  actionProbs: number[]
  dialogueTendency: number
  dialogueIntensity: number
  conflictFlag: boolean
  conflictIndex: number
}

export interface EmotionOutcome {
  behavior: EmotionBehavior
  state: EmotionState
}

/**
 * 处理一次情绪事件（有状态）。连续调用会累积记忆：
 * 强/重要事件固化为长期记忆，弱事件在工作记忆中逐渐遗忘。
 * `roleId` 指定角色，各角色记忆互相独立。
 */
export async function processEmotionEvent(
  roleId: number,
  event: EmotionEventInput,
): Promise<EmotionOutcome> {
  return invoke<EmotionOutcome>('emotion_process_event', { roleId, event })
}

/** 获取指定角色情绪引擎当前状态（不推进模型） */
export function getEmotionState(roleId: number): Promise<EmotionState> {
  return invoke<EmotionState>('emotion_get_state', { roleId })
}

/** 重置指定角色的情绪引擎：清空状态与工作/长期记忆 */
export function resetEmotionEngine(roleId: number): Promise<void> {
  return invoke('emotion_reset', { roleId })
}
