import { defineStore } from 'pinia'
import * as api from '@/api/services/script-editor'
import type {
  AssetIndex,
  ChapterContent,
  ChapterSummary,
  Diagnostic,
  ScriptCharacter,
  ScriptDetail,
  ScriptEventData,
  ScriptPackage,
  ScriptSchema,
  ValidationReport,
} from '@/api/services/script-editor'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useDialogStore } from '@/stores/modules/ui/dialog'

/** 撤销栈里的一帧：某个章节某一时刻的完整事件列表 */
interface UndoFrame {
  chapterId: string
  name?: string
  events: ScriptEventData[]
  /** 该帧对应的选中下标，撤销后光标回到原处 */
  selected: number
}

const UNDO_LIMIT = 100
const AUTOSAVE_DELAY = 800

/** 防抖计时器放模块级，不进 state —— 它不是响应式数据 */
let saveTimer: ReturnType<typeof setTimeout> | null = null

interface State {
  // ---- 常驻 ----
  schema: ScriptSchema | null
  scripts: ScriptPackage[]
  loading: boolean

  // ---- 当前打开的剧本 ----
  detail: ScriptDetail | null
  chapters: ChapterSummary[]
  assets: AssetIndex
  characters: ScriptCharacter[]

  // ---- 当前打开的章节 ----
  chapter: ChapterContent | null
  selectedEvent: number
  /** 复合块的展开状态，key 是块下标 */
  expandedGroups: Record<number, boolean>

  // ---- 编辑状态 ----
  dirty: boolean
  saving: boolean
  lastSavedAt: number | null
  undoStack: UndoFrame[]
  redoStack: UndoFrame[]

  // ---- 校验 ----
  report: ValidationReport | null
  validationOpen: boolean

  // ---- UI 偏好（唯一持久化的部分）----
  level: 'flow' | 'chapter'
  tab: 'flow' | 'config' | 'characters' | 'assets' | 'validate'
  foldCompounds: boolean
}

const emptyAssets = (): AssetIndex => ({
  background: [],
  music: [],
  sound: [],
  ambient: [],
  pic: [],
})

export const useScriptEditorStore = defineStore('script-editor', {
  state: (): State => ({
    schema: null,
    scripts: [],
    loading: false,

    detail: null,
    chapters: [],
    assets: emptyAssets(),
    characters: [],

    chapter: null,
    selectedEvent: 0,
    expandedGroups: {},

    dirty: false,
    saving: false,
    lastSavedAt: null,
    undoStack: [],
    redoStack: [],

    report: null,
    validationOpen: false,

    level: 'flow',
    tab: 'flow',
    foldCompounds: true,
  }),

  // 只持久化 UI 偏好。正文绝对不能进来 —— persist 插件每次 mutation 都全量
  // JSON.stringify 且没有防抖，把剧本树放进去等于每敲一个字符序列化一整棵树。
  persist: {
    key: 'lingchat-script-editor-ui',
    exclude: [
      'schema',
      'scripts',
      'loading',
      'detail',
      'chapters',
      'assets',
      'characters',
      'chapter',
      'selectedEvent',
      'expandedGroups',
      'dirty',
      'saving',
      'lastSavedAt',
      'undoStack',
      'redoStack',
      'report',
      'validationOpen',
    ],
  },

  getters: {
    scriptKey(): string | null {
      return this.detail?.package.key ?? null
    },

    /** 事件类型 → schema 定义 */
    eventSpecs(): Record<string, api.EventSpec> {
      const out: Record<string, api.EventSpec> = {}
      for (const e of this.schema?.events ?? []) out[e.typeKey] = e
      return out
    },

    /** character 字段的候选项：MAIN + 剧本内 NPC 的 roleKey */
    characterOptions(): string[] {
      return ['MAIN', ...this.characters.map((c) => c.roleKey)]
    },

    /** chapter 字段的候选项，末尾附一个「剧本结束」 */
    chapterOptions(): { value: string; label: string }[] {
      const list = this.chapters.map((c) => ({
        value: c.id,
        label: c.name ? `${c.name}（${c.id}）` : c.id,
      }))
      list.push({ value: 'end', label: '▸ 剧本结束' })
      return list
    },

    canUndo(): boolean {
      return this.undoStack.length > 0
    },
    canRedo(): boolean {
      return this.redoStack.length > 0
    },

    /** 当前章节的诊断，按事件下标归组，供时间线打标 */
    chapterDiagnostics(): Record<number, Diagnostic[]> {
      const out: Record<number, Diagnostic[]> = {}
      if (!this.report || !this.chapter) return out
      for (const d of this.report.diagnostics) {
        if (d.chapter !== this.chapter.id || d.eventIndex === undefined) continue
        ;(out[d.eventIndex] ||= []).push(d)
      }
      return out
    },

    hasBlockingErrors(): boolean {
      return (this.report?.errorCount ?? 0) > 0
    },
  },

  actions: {
    // ========================================================
    // 加载
    // ========================================================

    async init() {
      if (!this.schema) {
        try {
          this.schema = await api.getSchema()
        } catch (e) {
          this.notifyError('无法读取事件定义', e)
          return
        }
      }
      await this.refreshScripts()
    },

    async refreshScripts() {
      this.loading = true
      try {
        this.scripts = await api.listScripts()
      } catch (e) {
        this.notifyError('无法读取剧本列表', e)
      } finally {
        this.loading = false
      }
    },

    async openScript(key: string) {
      if (!(await this.confirmDiscardIfDirty())) return
      this.loading = true
      try {
        const detail = await api.readScript(key)
        this.detail = detail
        this.chapters = detail.chapters
        this.assets = detail.assets
        this.characters = detail.characters
        this.chapter = null
        this.resetHistory()
        this.level = 'flow'
        this.tab = 'flow'
        await this.runValidation({ silent: true })
      } catch (e) {
        this.notifyError('打开剧本失败', e)
      } finally {
        this.loading = false
      }
    },

    closeScript() {
      this.detail = null
      this.chapters = []
      this.assets = emptyAssets()
      this.characters = []
      this.chapter = null
      this.report = null
      this.resetHistory()
      this.level = 'flow'
    },

    async openChapter(chapterId: string) {
      const key = this.scriptKey
      if (!key) return
      if (!(await this.confirmDiscardIfDirty())) return
      try {
        this.chapter = await api.readChapter(key, chapterId)
        this.resetHistory()
        this.selectedEvent = this.firstStandaloneEvent()
        this.expandedGroups = {}
        this.level = 'chapter'
      } catch (e) {
        this.notifyError('打开章节失败', e)
      }
    },

    backToFlow() {
      this.level = 'flow'
    },

    // ========================================================
    // 编辑（全部走 pushHistory，保证可撤销）
    // ========================================================

    /** 在改动前记一帧。所有修改事件的操作都必须先调它。 */
    pushHistory() {
      if (!this.chapter) return
      this.undoStack.push({
        chapterId: this.chapter.id,
        name: this.chapter.name,
        events: JSON.parse(JSON.stringify(this.chapter.events)),
        selected: this.selectedEvent,
      })
      if (this.undoStack.length > UNDO_LIMIT) this.undoStack.shift()
      // 新的改动让 redo 失效
      this.redoStack = []
    },

    resetHistory() {
      this.undoStack = []
      this.redoStack = []
      this.dirty = false
      if (saveTimer) {
        clearTimeout(saveTimer)
        saveTimer = null
      }
    },

    undo() {
      if (!this.chapter || this.undoStack.length === 0) return
      const frame = this.undoStack.pop()!
      this.redoStack.push({
        chapterId: this.chapter.id,
        name: this.chapter.name,
        events: JSON.parse(JSON.stringify(this.chapter.events)),
        selected: this.selectedEvent,
      })
      this.applyFrame(frame)
    },

    redo() {
      if (!this.chapter || this.redoStack.length === 0) return
      const frame = this.redoStack.pop()!
      this.undoStack.push({
        chapterId: this.chapter.id,
        name: this.chapter.name,
        events: JSON.parse(JSON.stringify(this.chapter.events)),
        selected: this.selectedEvent,
      })
      this.applyFrame(frame)
    },

    applyFrame(frame: UndoFrame) {
      if (!this.chapter || frame.chapterId !== this.chapter.id) return
      this.chapter.name = frame.name
      this.chapter.events = frame.events
      this.selectedEvent = Math.min(frame.selected, Math.max(0, frame.events.length - 1))
      this.markDirty()
    },

    /** 新建一个符合 schema 的空事件骨架 */
    blankEvent(typeKey: string): ScriptEventData {
      const spec = this.eventSpecs[typeKey]
      const ev: ScriptEventData = { type: typeKey }
      if (!spec) return ev
      for (const f of spec.fields) {
        if (!f.required || !f.enabled) continue
        switch (f.kind) {
          case 'choice_options':
            ev[f.key] = [{ text: '', actions: [] }]
            break
          case 'branch_options':
            ev[f.key] = []
            break
          case 'var_options':
            ev[f.key] = [{ actions: [{ type: 'set_var', content: '' }] }]
            break
          case 'bool':
            ev[f.key] = false
            break
          case 'number':
            break
          case 'select':
            ev[f.key] = f.options?.[0] ?? ''
            break
          case 'character':
            ev[f.key] = 'MAIN'
            break
          default:
            ev[f.key] = ''
        }
      }
      return ev
    },

    insertEvent(typeKey: string, at?: number) {
      if (!this.chapter) return
      this.pushHistory()
      const index = at ?? this.chapter.events.length
      this.chapter.events.splice(index, 0, this.blankEvent(typeKey))
      this.selectedEvent = index
      this.markDirty()
    },

    removeEvent(index: number) {
      if (!this.chapter) return
      this.pushHistory()
      this.chapter.events.splice(index, 1)
      this.selectedEvent = Math.max(0, Math.min(index, this.chapter.events.length - 1))
      this.markDirty()
    },

    duplicateEvent(index: number) {
      if (!this.chapter) return
      const src = this.chapter.events[index]
      if (!src) return
      this.pushHistory()
      this.chapter.events.splice(index + 1, 0, JSON.parse(JSON.stringify(src)))
      this.selectedEvent = index + 1
      this.markDirty()
    },

    moveEvent(from: number, to: number) {
      if (!this.chapter) return
      if (from === to || from < 0 || to < 0) return
      if (from >= this.chapter.events.length || to >= this.chapter.events.length) return
      this.pushHistory()
      const [ev] = this.chapter.events.splice(from, 1)
      this.chapter.events.splice(to, 0, ev)
      this.selectedEvent = to
      this.markDirty()
    },

    /** 改事件的一个字段。空值一律删键，避免往 YAML 里写一堆空字符串。 */
    setEventField(index: number, key: string, value: unknown) {
      if (!this.chapter) return
      const ev = this.chapter.events[index]
      if (!ev) return
      this.pushHistory()
      const isEmpty =
        value === '' || value === null || value === undefined || value === '（不设置）'
      if (isEmpty) delete ev[key]
      else ev[key] = value
      this.markDirty()
    },

    setChapterName(name: string) {
      if (!this.chapter) return
      this.pushHistory()
      this.chapter.name = name.trim() === '' ? undefined : name
      this.markDirty()
    },

    // ========================================================
    // 保存
    // ========================================================

    markDirty() {
      this.dirty = true
      if (saveTimer) clearTimeout(saveTimer)
      // 防抖直写正式文件。Rust 侧是原子写 + .bak，配合撤销栈兜底反悔。
      saveTimer = setTimeout(() => {
        void this.save()
      }, AUTOSAVE_DELAY)
    },

    async save() {
      const key = this.scriptKey
      if (!key || !this.chapter || this.saving) return
      if (saveTimer) {
        clearTimeout(saveTimer)
        saveTimer = null
      }
      this.saving = true
      try {
        await api.writeChapter({
          key,
          chapterId: this.chapter.id,
          name: this.chapter.name,
          events: this.chapter.events,
          extra: this.chapter.extra,
        })
        this.dirty = false
        this.lastSavedAt = Date.now()
        // 保存后重跑校验：事件数、跳转关系都可能变了
        await this.runValidation({ silent: true })
        this.syncChapterSummary()
      } catch (e) {
        this.notifyError('自动保存失败，改动还在编辑器里', e)
      } finally {
        this.saving = false
      }
    },

    /** 把当前章节的事件数/显示名同步回流程图用的摘要列表 */
    syncChapterSummary() {
      if (!this.chapter) return
      const s = this.chapters.find((c) => c.id === this.chapter!.id)
      if (s) {
        s.eventCount = this.chapter.events.length
        s.name = this.chapter.name
      }
    },

    async confirmDiscardIfDirty(): Promise<boolean> {
      if (!this.dirty) return true
      // 有未保存改动时先落盘，而不是问用户要不要丢弃 —— 自动保存的语义下
      // 「丢弃」是个不该出现的选项。
      await this.save()
      return !this.dirty
    },

    // ========================================================
    // 章节增删改
    // ========================================================

    async createChapter(chapterId: string, name: string) {
      const key = this.scriptKey
      if (!key) return
      try {
        const created = await api.createChapter(key, chapterId, name)
        this.chapters.push({
          id: created.id,
          name: created.name,
          group: created.id.includes('/')
            ? created.id.slice(0, created.id.lastIndexOf('/'))
            : undefined,
          eventCount: created.events.length,
        })
        this.chapters.sort((a, b) => a.id.localeCompare(b.id))
        await this.runValidation({ silent: true })
        this.notifyOk('章节已创建', created.id)
      } catch (e) {
        this.notifyError('创建章节失败', e)
      }
    },

    async deleteChapter(chapterId: string) {
      const key = this.scriptKey
      if (!key) return
      const dialog = useDialogStore()
      const ok = await dialog.confirm(
        `确定删除章节「${chapterId}」吗？\n\n文件会被移到 Chapters/.trash/ 下，不会真正消失，但指向它的跳转会断链。`,
        '删除章节',
      )
      if (!ok) return
      try {
        await api.deleteChapter(key, chapterId)
        this.chapters = this.chapters.filter((c) => c.id !== chapterId)
        if (this.chapter?.id === chapterId) {
          this.chapter = null
          this.level = 'flow'
          this.resetHistory()
        }
        await this.runValidation({ silent: true })
      } catch (e) {
        this.notifyError('删除章节失败', e)
      }
    },

    // ========================================================
    // 校验 / 试玩
    // ========================================================

    async runValidation(opts: { silent?: boolean } = {}) {
      const key = this.scriptKey
      if (!key) return
      try {
        this.report = await api.validateScript(key)
        if (!opts.silent) this.validationOpen = true
      } catch (e) {
        this.notifyError('校验失败', e)
      }
    },

    /**
     * 试玩前的准备：先落盘、再校验、再让引擎重扫。
     *
     * 引擎只在启动时扫一次目录，不 rescan 的话新写的剧本根本不在列表里。
     * 有 error 时拦住 —— 保存不拦，试玩拦。
     */
    async preparePlaytest(): Promise<boolean> {
      const key = this.scriptKey
      if (!key) return false
      await this.save()
      await this.runValidation({ silent: true })
      if (this.hasBlockingErrors) {
        this.validationOpen = true
        this.notifyWarn(
          '还有问题没解决',
          `${this.report?.errorCount} 个错误会让剧本跑不通，先修掉再试玩`,
        )
        return false
      }
      try {
        await api.rescanScripts()
        await this.refreshScripts()
        return true
      } catch (e) {
        this.notifyError('重新加载剧本失败', e)
        return false
      }
    },

    // ========================================================
    // 素材 / 角色
    // ========================================================

    async uploadAsset(kind: api.AssetKind, fileName: string, data: Uint8Array) {
      const key = this.scriptKey
      if (!key) return
      try {
        const saved = await api.uploadAsset(key, kind, fileName, data)
        this.assets[kind].push(saved)
        this.assets[kind].sort()
        this.notifyOk('素材已导入', saved)
      } catch (e) {
        this.notifyError('导入素材失败', e)
      }
    },

    async createCharacter(folder: string, aiName: string, systemPrompt: string) {
      const key = this.scriptKey
      if (!key) return
      try {
        const c = await api.createCharacter(key, folder, aiName, systemPrompt)
        this.characters.push(c)
        this.notifyOk('角色已创建', `剧本里写 character: ${c.roleKey}`)
      } catch (e) {
        this.notifyError('创建角色失败', e)
      }
    },

    // ========================================================
    // 复合块折叠
    // ========================================================

    toggleGroup(index: number) {
      this.expandedGroups[index] = !this.expandedGroups[index]
    },

    expandAllGroups() {
      // 由组件把块下标传回来即可；这里直接全开
      const next: Record<number, boolean> = {}
      for (let i = 0; i < (this.chapter?.events.length ?? 0); i++) next[i] = true
      this.expandedGroups = next
    },

    /** 选中第一个没有被折叠进复合块的事件，避免「选中项看不见」 */
    firstStandaloneEvent(): number {
      return 0
    },

    // ========================================================
    // 提示
    // ========================================================

    notifyOk(title: string, message = '') {
      // skipTipsCheck 必传：没有 tips.txt 时 showNotification 会静默吞掉一切
      useUIStore().showNotification({ type: 'success', title, message, skipTipsCheck: true })
    },
    notifyWarn(title: string, message = '') {
      useUIStore().showNotification({ type: 'warning', title, message, skipTipsCheck: true })
    },
    notifyError(title: string, err: unknown) {
      useUIStore().showNotification({
        type: 'error',
        title,
        message: typeof err === 'string' ? err : String(err),
        skipTipsCheck: true,
        duration: 6000,
      })
    },
  },
})
