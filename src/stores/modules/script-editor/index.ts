import { defineStore } from 'pinia'
import * as api from '@/api/services/script-editor'
import type {
  AssetIndex,
  AssetKind,
  AssetScope,
  ChapterContent,
  ChapterEdge,
  ChapterSummary,
  Diagnostic,
  EventSpec,
  ScriptCharacter,
  ScriptDetail,
  ScriptEventData,
  ScriptPackage,
  ScriptSchema,
  ValidationReport,
} from '@/api/services/script-editor'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { firstVisibleIndex } from '@/composables/useEventFolding'

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
/** 校验比保存重得多（要扫全部剧本），所以单独用更长的防抖 */
const VALIDATE_DELAY = 2500

/**
 * 这些都不是响应式数据，放模块级而不是 state。
 *
 * `revision` 是防丢改动的关键：保存是异步的，落盘期间用户可能又改了东西。
 * 早先的实现在写成功后无条件 `dirty = false`，那次编辑就既没落盘、又显示
 * 「已保存」，切章节时被 readChapter 覆盖掉 —— 静默丢失。
 */
let saveTimer: ReturnType<typeof setTimeout> | null = null
let validateTimer: ReturnType<typeof setTimeout> | null = null
let revision = 0
let savePending = false
/** 请求代次，防止快速切换时先发的响应后到覆盖掉后发的 */
let openSeq = 0
let validateSeq = 0

interface State {
  // ---- 常驻 ----
  schema: ScriptSchema | null
  scripts: ScriptPackage[]
  loading: boolean

  // ---- 当前打开的剧本 ----
  /** 单一真相源。章节 / 素材 / 角色都从这里读，不再另存一份副本 */
  detail: ScriptDetail | null
  /** 全局素材，与剧本自带素材分开展示 */
  globalAssets: AssetIndex

  // ---- 当前打开的章节 ----
  chapter: ChapterContent | null
  selectedEvent: number

  // ---- 编辑状态 ----
  dirty: boolean
  saving: boolean
  lastSavedAt: number | null
  undoStack: UndoFrame[]
  redoStack: UndoFrame[]

  // ---- 校验 ----
  report: ValidationReport | null

  // ---- 试玩 ----
  previewing: boolean

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
    globalAssets: emptyAssets(),

    chapter: null,
    selectedEvent: 0,

    dirty: false,
    saving: false,
    lastSavedAt: null,
    undoStack: [],
    redoStack: [],

    report: null,
    previewing: false,

    level: 'flow',
    tab: 'flow',
    foldCompounds: true,
  }),

  // 只持久化 UI 偏好。用白名单式的 exclude 很容易漏 —— 新增 state 字段会
  // 默认被持久化，与「正文绝对不进 localStorage」的意图相反。这里把所有
  // 非偏好字段都显式列出来，新增字段时务必同步。
  // （persist 插件每次 mutation 都全量 JSON.stringify 且无防抖。）
  persist: {
    key: 'lingchat-script-editor-ui',
    exclude: [
      'schema',
      'scripts',
      'loading',
      'detail',
      'globalAssets',
      'chapter',
      'selectedEvent',
      'dirty',
      'saving',
      'lastSavedAt',
      'undoStack',
      'redoStack',
      'report',
      'previewing',
    ],
  },

  getters: {
    scriptKey(): string | null {
      return this.detail?.package.key ?? null
    },

    chapters(): ChapterSummary[] {
      return this.detail?.chapters ?? []
    },

    assets(): AssetIndex {
      return this.detail?.assets ?? emptyAssets()
    },

    characters(): ScriptCharacter[] {
      return this.detail?.characters ?? []
    },

    /** 事件类型 → schema 定义 */
    eventSpecs(): Record<string, EventSpec> {
      const out: Record<string, EventSpec> = {}
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

    /** 开场章节 id */
    introChapter(): string {
      const raw = this.detail?.storyConfig?.intro_chapter
      return typeof raw === 'string' ? raw.replace(/\.yaml$/, '') : 'main'
    },

    /** 章节跳转边，来自校验报告 */
    edges(): ChapterEdge[] {
      return this.report?.edges ?? []
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

    /** 按章节聚合的错误/警告数，供校验页与流程图显示 */
    diagnosticsByChapter(): Record<string, { errors: number; warns: number; infos: number }> {
      const out: Record<string, { errors: number; warns: number; infos: number }> = {}
      for (const c of this.chapters) out[c.id] = { errors: 0, warns: 0, infos: 0 }
      for (const d of this.report?.diagnostics ?? []) {
        if (!d.chapter) continue
        const slot = (out[d.chapter] ||= { errors: 0, warns: 0, infos: 0 })
        if (d.severity === 'error') slot.errors++
        else if (d.severity === 'warn') slot.warns++
        else slot.infos++
      }
      return out
    },

    /** 剧本级（不属于任何章节）的诊断 */
    scriptDiagnostics(): Diagnostic[] {
      return (this.report?.diagnostics ?? []).filter((d) => !d.chapter)
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
      void this.refreshGlobalAssets()
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

    async refreshGlobalAssets() {
      try {
        this.globalAssets = await api.listGlobalAssets()
      } catch (e) {
        // 全局素材读不到不该阻塞编辑，静默降级
        console.warn('读取全局素材失败:', e)
      }
    },

    async openScript(key: string) {
      await this.flushPendingSave()
      const seq = ++openSeq
      this.loading = true
      try {
        const detail = await api.readScript(key)
        if (seq !== openSeq) return
        this.detail = detail
        this.chapter = null
        this.resetHistory()
        this.level = 'flow'
        this.tab = 'flow'
        await this.runValidation()
      } catch (e) {
        if (seq === openSeq) this.notifyError('打开剧本失败', e)
      } finally {
        if (seq === openSeq) this.loading = false
      }
    },

    closeScript() {
      this.detail = null
      this.chapter = null
      this.report = null
      this.resetHistory()
      this.level = 'flow'
    },

    /**
     * 删除整个剧本包。后端把目录整体移到 `game_data/.script_trash/`，不真删。
     *
     * 与 deleteChapter 一样必须先问一遍：这里删掉的是作者的全部工作量。
     */
    async deleteScript(key: string, displayName: string) {
      const dialog = useDialogStore()
      const ok = await dialog.confirm(
        `确定删除剧本「${displayName}」吗？\n\n` +
          '整个目录（章节、素材、角色）会被移到 game_data/.script_trash/ 下，' +
          '不会真正消失，但引擎里会立刻消失。若它是某个角色的羁绊冒险，角色卡上也会不见。',
        '删除剧本',
      )
      if (!ok) return
      try {
        if (this.scriptKey === key) this.closeScript()
        await api.deleteScript(key)
        await this.refreshScripts()
        // 引擎内存里还留着这个剧本，不同步的话主菜单仍然列得出来
        await this.syncEngine()
        this.notifyOk('剧本已删除', '可以在 game_data/.script_trash/ 里找回')
      } catch (e) {
        this.notifyError('删除剧本失败', e)
      }
    },

    /**
     * 把磁盘上的改动同步进引擎内存。
     *
     * 引擎只在启动时扫一次剧本目录，所以作者在编辑器里改完之后，
     * 主菜单的剧本列表 / 角色卡的羁绊冒险仍然是旧的，得重启应用才生效。
     * 离开编辑器和删除剧本后各同步一次，正好覆盖「编辑完就去玩」这条路径。
     *
     * 失败不弹窗：这是收尾动作，作者此刻已经在往外走，弹窗只会挡路。
     * 真的没同步上，最坏结果也只是需要重启一次。
     */
    async syncEngine() {
      try {
        await api.rescanScripts()
      } catch (e) {
        console.warn('同步剧本到引擎失败，可能需要重启应用:', e)
      }
    },

    /** 返回是否成功打开 —— 调用方（诊断跳转）需要据此决定要不要继续 */
    async openChapter(chapterId: string): Promise<boolean> {
      const key = this.scriptKey
      if (!key) return false
      await this.flushPendingSave()
      const seq = ++openSeq
      try {
        const content = await api.readChapter(key, chapterId)
        if (seq !== openSeq) return false
        this.chapter = content
        this.resetHistory()
        // 选中第一个没被折叠进复合块的事件。官方剧本每章开头都是一个转场块，
        // 直接选 0 会出现「右侧显示字段、左侧那行是收起的转场」。
        this.selectedEvent = firstVisibleIndex(content.events, this.foldCompounds)
        this.level = 'chapter'
        return true
      } catch (e) {
        if (seq === openSeq) this.notifyError('打开章节失败', e)
        return false
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
      savePending = false
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
      // 章节结束默认指向「剧本结束」，否则一插入就报「linear 但没写下一章」
      if (typeKey === 'chapter_end') ev.next_chapter = 'end'
      return ev
    },

    /**
     * 插入事件。
     *
     * 默认插到**最后一条 chapter_end 之前**而不是数组末尾 —— 新章节自带一条
     * chapter_end，插到它后面每次都会立刻报「章节结束之后还有事件，永远不会执行」，
     * 作者得先看到一条警告再手动往上挪。
     */
    insertEvent(typeKey: string, at?: number) {
      if (!this.chapter) return
      this.pushHistory()
      const events = this.chapter.events
      const index = at ?? this.defaultInsertIndex()
      events.splice(index, 0, this.blankEvent(typeKey))
      this.selectedEvent = index
      this.markDirty()
    },

    defaultInsertIndex(): number {
      const events = this.chapter?.events ?? []
      for (let i = events.length - 1; i >= 0; i--) {
        if (events[i]?.type === 'chapter_end') return i
      }
      return events.length
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
      if (value === '' || value === null || value === undefined) delete ev[key]
      else ev[key] = value
      this.markDirty()
    },

    /** 整体替换一个事件（换类型时用） */
    replaceEvent(index: number, next: ScriptEventData) {
      if (!this.chapter) return
      this.pushHistory()
      this.chapter.events[index] = next
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
      revision++
      if (saveTimer) clearTimeout(saveTimer)
      // 防抖直写正式文件。Rust 侧是原子写 + .bak，配合撤销栈兜底反悔。
      saveTimer = setTimeout(() => {
        void this.save()
      }, AUTOSAVE_DELAY)
      this.scheduleValidation()
    },

    async save() {
      const key = this.scriptKey
      if (!key || !this.chapter) return
      if (saveTimer) {
        clearTimeout(saveTimer)
        saveTimer = null
      }
      // 已经有一次落盘在飞：记下来，等它结束后再写一次，而不是直接丢掉
      if (this.saving) {
        savePending = true
        return
      }

      this.saving = true
      const rev = revision
      try {
        await api.writeChapter({
          key,
          chapterId: this.chapter.id,
          name: this.chapter.name,
          events: this.chapter.events,
          extra: this.chapter.extra,
        })
        // 只有期间没有新改动才算干净 —— 否则那次编辑还没落盘
        if (rev === revision) this.dirty = false
        this.lastSavedAt = Date.now()
        this.syncChapterSummary()
      } catch (e) {
        this.notifyError('自动保存失败，改动还在编辑器里', e)
      } finally {
        this.saving = false
        if (savePending) {
          savePending = false
          void this.save()
        }
      }
    },

    /** 把当前章节的事件数/显示名同步回流程图用的摘要 */
    syncChapterSummary() {
      if (!this.chapter || !this.detail) return
      const s = this.detail.chapters.find((c) => c.id === this.chapter!.id)
      if (s) {
        s.eventCount = this.chapter.events.length
        s.name = this.chapter.name
      }
    },

    /**
     * 把待写入的改动立刻落盘。
     *
     * 名字刻意不叫 confirmDiscard*：自动保存的语义下「丢弃」不该是一个选项，
     * 这里从不询问用户。
     */
    async flushPendingSave(): Promise<void> {
      if (!this.dirty && !savePending) return
      await this.save()
    },

    // ========================================================
    // 章节增删改与重排
    // ========================================================

    async createChapter(chapterId: string, name: string) {
      const key = this.scriptKey
      if (!key || !this.detail) return
      try {
        const created = await api.createChapter(key, chapterId, name)
        this.detail.chapters.push({
          id: created.id,
          name: created.name,
          group: created.id.includes('/')
            ? created.id.slice(0, created.id.lastIndexOf('/'))
            : undefined,
          eventCount: created.events.length,
        })
        this.detail.chapters.sort((a, b) => a.id.localeCompare(b.id))
        await this.runValidation()
        this.notifyOk('章节已创建', created.id)
      } catch (e) {
        this.notifyError('创建章节失败', e)
      }
    },

    async deleteChapter(chapterId: string) {
      const key = this.scriptKey
      if (!key || !this.detail) return
      const dialog = useDialogStore()
      const ok = await dialog.confirm(
        `确定删除章节「${chapterId}」吗？\n\n文件会被移到 Chapters/.trash/ 下，不会真正消失，但指向它的跳转会断链。`,
        '删除章节',
      )
      if (!ok) return
      try {
        await api.deleteChapter(key, chapterId)
        this.detail.chapters = this.detail.chapters.filter((c) => c.id !== chapterId)
        if (this.chapter?.id === chapterId) {
          this.chapter = null
          this.level = 'flow'
          this.resetHistory()
        }
        await this.runValidation()
      } catch (e) {
        this.notifyError('删除章节失败', e)
      }
    },

    /**
     * 重排一条 linear 链。
     *
     * 章节先后是 chapter_end.next_chapter 串出来的，所以这里做的是重新接线，
     * 不是改文件名顺序。分支章节会被后端拒绝。
     */
    async reorderChapters(order: string[]) {
      const key = this.scriptKey
      if (!key || order.length < 2) return
      await this.flushPendingSave()
      try {
        await api.reorderChapters(key, order)
        // 当前打开的章节可能就是被改写的那个，重读一次免得覆盖回去
        if (this.chapter) {
          const reread = await api.readChapter(key, this.chapter.id)
          this.chapter = reread
          this.resetHistory()
        }
        await this.runValidation()
        this.notifyOk('章节顺序已更新')
      } catch (e) {
        this.notifyError('重排章节失败', e)
      }
    },

    // ========================================================
    // 校验
    // ========================================================

    /**
     * 校验要扫全部剧本（为了查剧本名重复）再逐章读盘，比保存重得多，
     * 所以用比自动保存更长的防抖，而不是每次落盘都跟着跑一遍。
     */
    scheduleValidation() {
      if (validateTimer) clearTimeout(validateTimer)
      validateTimer = setTimeout(() => {
        void this.runValidation()
      }, VALIDATE_DELAY)
    },

    async runValidation() {
      const key = this.scriptKey
      if (!key) return
      if (validateTimer) {
        clearTimeout(validateTimer)
        validateTimer = null
      }
      const seq = ++validateSeq
      try {
        const report = await api.validateScript(key)
        if (seq === validateSeq) this.report = report
      } catch (e) {
        if (seq === validateSeq) this.notifyError('校验失败', e)
      }
    },

    // ========================================================
    // 试玩
    // ========================================================

    /**
     * 在编辑器内试玩。
     *
     * 保存不拦 error，试玩拦 —— 跑一个已知跑不通的剧本只会浪费作者时间。
     * `fromChapter` 留空则从开场章节开始。
     */
    async startPreview(fromChapter?: string): Promise<boolean> {
      const key = this.scriptKey
      if (!key) return false
      await this.flushPendingSave()
      await this.runValidation()
      if (this.hasBlockingErrors) {
        this.tab = 'validate'
        this.notifyWarn(
          '还有问题没解决',
          `${this.report?.errorCount} 个错误会让剧本跑不通，先修掉再试玩`,
        )
        return false
      }
      try {
        await api.startPreview(key, fromChapter)
        this.previewing = true
        await this.refreshScripts()
        return true
      } catch (e) {
        this.notifyError('试玩启动失败', e)
        return false
      }
    },

    async stopPreview() {
      if (!this.previewing) return
      this.previewing = false
      try {
        await api.stopPreview()
      } catch (e) {
        console.warn('停止试玩失败:', e)
      }
    },

    // ========================================================
    // 素材 / 角色
    // ========================================================

    /**
     * 导入素材。`scope` 决定落点：剧本独有（随剧本分发）或全局（所有剧本共享）。
     * 返回落盘后的文件名 —— Rust 会做一次名称清洗，可能与源文件名不同。
     */
    async uploadAsset(
      kind: AssetKind,
      scope: AssetScope,
      srcPath: string,
    ): Promise<string | null> {
      const key = this.scriptKey
      if (!key) return null
      try {
        const saved = await api.uploadAsset(key, kind, scope, srcPath)
        if (scope === 'global') {
          await this.refreshGlobalAssets()
        } else if (this.detail) {
          this.detail.assets[kind].push(saved)
          this.detail.assets[kind].sort()
        }
        this.notifyOk(
          scope === 'global' ? '已导入为全局素材' : '已导入为剧本素材',
          saved,
        )
        return saved
      } catch (e) {
        this.notifyError('导入素材失败', e)
        return null
      }
    },

    async createCharacter(folder: string, aiName: string, systemPrompt: string) {
      const key = this.scriptKey
      if (!key || !this.detail) return
      try {
        const c = await api.createCharacter(key, folder, aiName, systemPrompt)
        this.detail.characters.push(c)
        this.notifyOk('角色已创建', `剧本里写 character: ${c.roleKey}`)
      } catch (e) {
        this.notifyError('创建角色失败', e)
      }
    },

    // ========================================================
    // 剧本设置
    // ========================================================

    async saveStoryConfig(config: Record<string, unknown>) {
      const key = this.scriptKey
      if (!key || !this.detail) return
      try {
        await api.writeStoryConfig(key, config)
        this.detail.storyConfig = config
        await this.refreshScripts()
        await this.runValidation()
        this.notifyOk('剧本设置已保存')
      } catch (e) {
        this.notifyError('保存剧本设置失败', e)
      }
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
