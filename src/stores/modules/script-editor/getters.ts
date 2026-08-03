/**
 * 剧本编辑器 store —— getters（setup 风格：computed）。
 *
 * 接收 useEditorState 的 ref 集合，返回一组 computed。读 ref 走 .value。
 * 几个 getter 原本互相引用（characterOptions 用 characters），这里就地内联，
 * 避免跨 getter 调用。
 */
import { computed } from 'vue'
import type {
  AssetIndex,
  ChapterEdge,
  ChapterSummary,
  Diagnostic,
  EventSpec,
  ScriptCharacter,
} from '@/api/services/script-editor'
import { emptyAssets, useEditorState } from './state'

type StateRefs = ReturnType<typeof useEditorState>

export const useEditorGetters = (s: StateRefs) => {
  const scriptKey = computed(() => s.detail.value?.package.key ?? null)

  const chapters = computed<ChapterSummary[]>(() => s.detail.value?.chapters ?? [])

  const assets = computed<AssetIndex>(() => s.detail.value?.assets ?? emptyAssets())

  const characters = computed<ScriptCharacter[]>(() => s.detail.value?.characters ?? [])

  /** 事件类型 → schema 定义 */
  const eventSpecs = computed<Record<string, EventSpec>>(() => {
    const out: Record<string, EventSpec> = {}
    for (const e of s.schema.value?.events ?? []) out[e.typeKey] = e
    return out
  })

  /** character 字段的候选项：MAIN + 剧本内 NPC 的 roleKey */
  const characterOptions = computed<string[]>(() => [
    'MAIN',
    ...(s.detail.value?.characters ?? []).map((c) => c.roleKey),
  ])

  /** chapter 字段的候选项，末尾附一个「剧本结束」 */
  const chapterOptions = computed<{ value: string; label: string }[]>(() => {
    const list = (s.detail.value?.chapters ?? []).map((c) => ({
      value: c.id,
      label: c.name ? `${c.name}（${c.id}）` : c.id,
    }))
    list.push({ value: 'end', label: '▸ 剧本结束' })
    return list
  })

  /** 开场章节 id */
  const introChapter = computed<string>(() => {
    const raw = s.detail.value?.storyConfig?.intro_chapter
    return typeof raw === 'string' ? raw.replace(/\.yaml$/, '') : 'main'
  })

  /** 章节跳转边，来自校验报告 */
  const edges = computed<ChapterEdge[]>(() => s.report.value?.edges ?? [])

  const canUndo = computed(() => s.undoStack.value.length > 0)
  const canRedo = computed(() => s.redoStack.value.length > 0)

  /** 当前章节的诊断，按事件下标归组，供时间线打标 */
  const chapterDiagnostics = computed<Record<number, Diagnostic[]>>(() => {
    const out: Record<number, Diagnostic[]> = {}
    if (!s.report.value || !s.chapter.value) return out
    for (const d of s.report.value.diagnostics) {
      if (d.chapter !== s.chapter.value.id || d.eventIndex === undefined) continue
      ;(out[d.eventIndex] ||= []).push(d)
    }
    return out
  })

  /** 按章节聚合的错误/警告数，供校验页与流程图显示 */
  const diagnosticsByChapter = computed<
    Record<string, { errors: number; warns: number; infos: number }>
  >(() => {
    const out: Record<string, { errors: number; warns: number; infos: number }> = {}
    for (const c of s.detail.value?.chapters ?? []) out[c.id] = { errors: 0, warns: 0, infos: 0 }
    for (const d of s.report.value?.diagnostics ?? []) {
      if (!d.chapter) continue
      const slot = (out[d.chapter] ||= { errors: 0, warns: 0, infos: 0 })
      if (d.severity === 'error') slot.errors++
      else if (d.severity === 'warn') slot.warns++
      else slot.infos++
    }
    return out
  })

  /** 剧本级（不属于任何章节）的诊断 */
  const scriptDiagnostics = computed<Diagnostic[]>(() =>
    (s.report.value?.diagnostics ?? []).filter((d) => !d.chapter),
  )

  /** 全剧本出现过的变量名，供变量编辑器做输入补全 */
  const variables = computed<string[]>(() => s.report.value?.variables ?? [])

  const hasBlockingErrors = computed(() => (s.report.value?.errorCount ?? 0) > 0)

  return {
    scriptKey,
    chapters,
    assets,
    characters,
    eventSpecs,
    characterOptions,
    chapterOptions,
    introChapter,
    edges,
    canUndo,
    canRedo,
    chapterDiagnostics,
    diagnosticsByChapter,
    scriptDiagnostics,
    variables,
    hasBlockingErrors,
  }
}
