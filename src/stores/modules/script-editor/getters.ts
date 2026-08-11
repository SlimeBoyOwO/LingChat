/**
 * 剧本编辑器 store —— getters（setup 风格：computed）。
 *
 * 接收 useEditorState 的 ref 集合，返回一组 computed。读 ref 走 .value。
 * 几个 getter 原本互相引用（characterOptions 用 characters），这里就地内联，
 * 避免跨 getter 调用。
 */
import { computed } from 'vue'
import { i18n } from '@/locales'
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
  /** 章节下拉选项文案（剧本结束）走 i18n */
  const t = i18n.global.t
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

  /**
   * character 字段的候选项：NPC +（羁绊剧本的）MAIN。
   * value 是引擎认的 roleKey（script_role_key，缺省回落目录名），
   * label 显示角色名字（name/aiName）——避免作者看到目录键却认不出是谁；
   * 重复 roleKey 去重。
   * 独立剧本（未绑定羁绊人物）不提供 MAIN 选项：引擎对 character 留空
   * 回落 MAIN（当前主角），需要主角台词时留空即可（校验器不拦）。
   */
  const characterOptions = computed<{ value: string; label: string }[]>(() => {
    const out: { value: string; label: string }[] = []
    if (s.detail.value?.package.boundCharacterFolder) {
      out.push({ value: 'MAIN', label: mainRoleDisplayName.value })
    }
    const seen = new Set<string>()
    for (const c of s.detail.value?.characters ?? []) {
      if (seen.has(c.roleKey)) continue
      seen.add(c.roleKey)
      out.push({ value: c.roleKey, label: c.aiName || c.roleKey })
    }
    return out
  })

  /**
   * MAIN 的展示名：优先用试玩可行性解析出的绑定角色名，其次按
   * bound_character_folder 在全局角色库里匹配 aiName，都没有才回退词条。
   * 用于时间线摘要、事件属性面板等处的「MAIN」显示。
   */
  const mainRoleDisplayName = computed<string>(() => {
    const bound = s.detail.value?.package.boundCharacterFolder
    if (bound) {
      // 羁绊剧本：优先全局角色库的实时扫描结果（editor_list_global_characters
      // 直接读 settings.yml 的 name/ai_name）；readiness 的 DB name 是角色
      // 初始化时写入的 title（非显示名），仅作最后兜底（后端 role_name_of
      // 已改为读 settings.yml，返回的也是显示名）
      const gc = s.globalCharacters.value.find((g) => g.folder === bound)
      if (gc?.aiName) return gc.aiName
      const fromReadiness = s.readiness.value?.mainRoleName
      if (fromReadiness) return fromReadiness
    }
    // 独立剧本（未绑定）：MAIN = 运行时主角，readiness 的 DB name 是 title 且
    // 不可靠（还可能是上次会话残留的主角），不采用——只落到玩家名/字面 MAIN
    // 剧本显式设置了玩家名时，MAIN 显示玩家名（比字面 MAIN 更接近「名字」）
    const ss = s.detail.value?.storyConfig as Record<string, unknown> | undefined
    const scriptUserName = (ss?.script_settings as Record<string, unknown> | undefined)
      ?.user_name
    if (typeof scriptUserName === 'string' && scriptUserName.trim()) {
      return scriptUserName.trim()
    }
    return i18n.global.t('scriptEditor.fieldRow.mainRole')
  })

  /** chapter 字段的候选项，末尾附一个「剧本结束」 */
  const chapterOptions = computed<{ value: string; label: string }[]>(() => {
    const list = (s.detail.value?.chapters ?? []).map((c) => ({
      value: c.id,
      label: c.name ? `${c.name}（${c.id}）` : c.id,
    }))
    list.push({ value: 'end', label: `▸ ${t('scriptEditor.chapterFlow.end')}` })
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
    mainRoleDisplayName,
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
