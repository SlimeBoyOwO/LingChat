import { invoke } from '@tauri-apps/api/core'

/** 上下文用量信息（get_context_usage） */
export interface ContextUsageInfo {
  /** 估算的当前上下文用量（tokens） */
  usedTokens: number
  /** 当前对话模型的上下文窗口（tokens） */
  windowTokens: number
  /** 用量百分比（可超 100） */
  percent: number
  /** 当前存档的台词总数 */
  lineCount: number
  /** 是否有 provider 实测锚点 */
  hasMeasuredAnchor: boolean
  /** 压缩摘要是否生效中 */
  compacted: boolean
  /** 摘要覆盖到的台词条数 */
  compactedUpto: number
  /** 自动压缩开关 */
  autoCompact: boolean
}

/** 压缩结果（compact_context） */
export interface CompactOutcome {
  compactedLines: number
  keptLines: number
  usedTokensAfter: number
  message: string
}

export async function getContextUsage(): Promise<ContextUsageInfo> {
  return invoke('get_context_usage')
}

export async function compactContext(): Promise<CompactOutcome> {
  return invoke('compact_context')
}
