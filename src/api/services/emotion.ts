import { invoke } from '@tauri-apps/api/core'

/**
 * 情绪分析引擎统一为单个 ONNX 模型（sherpa-onnx DeBERTa 19 分类）。
 * 旧版 Python 后端的 "9d" 引擎与 "关闭" 选项已随后端移除。
 */
export type EmotionModelType = 'onnx'

export interface EmotionModelInfo {
  current: EmotionModelType
  available: EmotionModelType[]
}

/**
 * 获取当前情绪模型类型。后端只保留单个 ONNX 分类器，恒为 "onnx"。
 */
export async function getEmotionModelType(): Promise<EmotionModelType> {
  try {
    await invoke('get_setting_by_key', {
      key: 'features.emotion_model_type',
    })
  } catch {
    // 设置键不存在时忽略
  }
  return 'onnx'
}

export const EMOTION_MODEL_LABELS: Record<
  EmotionModelType,
  { label: string; description: string }
> = {
  onnx: {
    label: '经典 ONNX 模型',
    description: '基于 BERT 的 19 分类情绪模型',
  },
}
