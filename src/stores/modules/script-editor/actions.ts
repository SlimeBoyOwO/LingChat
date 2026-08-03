/**
 * 剧本编辑器 store —— actions（setup 风格：普通函数）。
 *
 * 与 game store 的 option 风格不同：script-editor 的 action 大量互调兄弟方法，
 * option 风格里拆出 actions 会循环引用导致 store 类型退化（已验证失败），故整体转
 * setup——这里是普通函数，this.X 已改为 s.X.value / g.X.value / 直接调用兄弟函数。
 *
 * 模块级可变量（saveTimer/revision/…）仍放模块级：单一 store 实例，不需 per-instance。
 */
import * as api from '@/api/services/script-editor'
import * as achievementApi from '@/api/services/achievement'
import type {
  AssetKind,
  AssetScope,
  ChapterSummary,
  GlobalCharacter,
  ScriptEventData,
} from '@/api/services/script-editor'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { firstVisibleIndex } from '@/composables/useEventFolding'
import { eventQueue } from '@/core/events/event-queue'
import { AUTOSAVE_DELAY, UNDO_LIMIT, VALIDATE_DELAY, useEditorState } from './state'
import type { UndoFrame } from './state'
import { useEditorGetters } from './getters'
import { particleEffectOptions, canonicalEffectKey } from '@/components/game/standard/particles'

type StateRefs = ReturnType<typeof useEditorState>
type Getters = ReturnType<typeof useEditorGetters>

/**
 * 把 schema 里由后端常量驱动的下拉，改由前端注册表驱动。
 * 目前只有「背景特效」：用前端粒子注册表覆盖该字段 options，新增粒子只改前端。
 */
function applyFrontendOverrides(schema: { events: { typeKey: string; fields: { key: string; options?: unknown[] }[] }[] } | null) {
  if (!schema) return
  const effectField = schema.events
    .find((e) => e.typeKey === 'background_effect')
    ?.fields.find((f) => f.key === 'effect')
  if (effectField) {
    effectField.options = particleEffectOptions()
  }
}

/**
 * 这些都不是响应式数据，放模块级而不是 state。
 * `revision` 是防丢改动的关键：保存是异步的，落盘期间用户可能又改了东西。
 */
let saveTimer: ReturnType<typeof setTimeout> | null = null
let validateTimer: ReturnType<typeof setTimeout> | null = null
let revision = 0
let savePending = false
/** 请求代次，防止快速切换时先发的响应后到覆盖掉后发的 */
let openSeq = 0
let validateSeq = 0

export const useEditorActions = (s: StateRefs, g: Getters) => {
  async function init() {
    if (!s.schema.value) {
      try {
        s.schema.value = await api.getSchema()
        applyFrontendOverrides(s.schema.value)
      } catch (e) {
        notifyError('无法读取事件定义', e)
        return
      }
    }
    await refreshScripts()
    void refreshGlobalAssets()
  }

  async function refreshScripts() {
    s.loading.value = true
    try {
      s.scripts.value = await api.listScripts()
    } catch (e) {
      notifyError('无法读取剧本列表', e)
    } finally {
      s.loading.value = false
    }
  }

  async function refreshGlobalAssets() {
    try {
      s.globalAssets.value = await api.listGlobalAssets()
    } catch (e) {
      // 全局素材读不到不该阻塞编辑，静默降级
      console.warn('读取全局素材失败:', e)
    }
  }

  async function openScript(key: string) {
    await flushPendingSave()
    const seq = ++openSeq
    s.loading.value = true
    try {
      const detail = await api.readScript(key)
      if (seq !== openSeq) return
      s.detail.value = detail
      s.chapter.value = null
      resetHistory()
      s.level.value = 'flow'
      s.tab.value = 'flow'
      void refreshGlobalCharacters()
      void loadAchievements()
      void checkReadiness()
      await runValidation()
    } catch (e) {
      if (seq === openSeq) notifyError('打开剧本失败', e)
    } finally {
      if (seq === openSeq) s.loading.value = false
    }
  }

  function closeScript() {
    s.detail.value = null
    s.chapter.value = null
    s.report.value = null
    resetHistory()
    s.level.value = 'flow'
  }

  /**
   * 删除整个剧本包。
   *
   * 与 deleteChapter 一样必须先问一遍：这里删掉的是作者的全部工作量。
   */
  async function deleteScript(key: string, displayName: string) {
    const dialog = useDialogStore()
    const ok = await dialog.confirm(
      `确定删除剧本「${displayName}」吗？\n\n` +
        '整个目录（章节、素材、角色）将被永久删除，无法恢复。' +
        '若它是某个角色的羁绊冒险，角色卡上也会消失。',
      '删除剧本',
    )
    if (!ok) return
    try {
      if (g.scriptKey.value === key) closeScript()
      await api.deleteScript(key)
      await refreshScripts()
      // 引擎内存里还留着这个剧本，不同步的话主菜单仍然列得出来
      await syncEngine()
      notifyOk('剧本已删除', `${displayName} 已被永久删除`)
    } catch (e) {
      notifyError('删除剧本失败', e)
    }
  }

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
  async function syncEngine() {
    try {
      await api.rescanScripts()
    } catch (e) {
      console.warn('同步剧本到引擎失败，可能需要重启应用:', e)
    }
  }

  /** 返回是否成功打开 —— 调用方（诊断跳转）需要据此决定要不要继续 */
  async function openChapter(chapterId: string): Promise<boolean> {
    const key = g.scriptKey.value
    if (!key) return false
    await flushPendingSave()
    const seq = ++openSeq
    try {
      const content = await api.readChapter(key, chapterId)
      if (seq !== openSeq) return false
      // 大小写自动纠错（上游要求「前端识别上实现自动纠错」）：AI/手改产出的
      // starfield → StarField。命中且与原值不同就改回，并标脏让自动保存落盘。
      // 未命中的（真未知特效）不强行改写，留给 validate/runtime warn。
      let effectCorrected = false
      for (const ev of content.events) {
        if (ev.type === 'background_effect' && typeof ev.effect === 'string') {
          const canon = canonicalEffectKey(ev.effect)
          if (canon && canon !== ev.effect) {
            ev.effect = canon
            effectCorrected = true
          }
        }
      }
      s.chapter.value = content
      resetHistory()
      // 纠错过的标记脏，让防抖自动保存把规范写法落盘（下次校验/运行就不再 warn）
      if (effectCorrected) markDirty()
      // 选中第一个没被折叠进复合块的事件。官方剧本每章开头都是一个转场块，
      // 直接选 0 会出现「右侧显示字段、左侧那行是收起的转场」。
      s.selectedEvent.value = firstVisibleIndex(content.events, s.foldCompounds.value)
      s.level.value = 'chapter'
      return true
    } catch (e) {
      if (seq === openSeq) notifyError('打开章节失败', e)
      return false
    }
  }

  /**
   * 回到章节流程图。
   *
   * 刻意先落盘再校验：流程图画的是 `report.edges`，而 `runValidation` 读的是
   * **磁盘**。改完「下一章」立刻退回来时，自动保存的 800ms 防抖和校验的
   * 2.5s 防抖都还没到点，图上仍是旧连线 —— 作者会以为改动没生效。
   * 这里把两步都强制走一遍，代价是退回时多等一下，换来「看到的就是真的」。
   */
  async function backToFlow() {
    s.level.value = 'flow'
    await flushPendingSave()
    await runValidation()
  }

  // ========================================================
  // 编辑（全部走 pushHistory，保证可撤销）
  // ========================================================

  /** 在改动前记一帧。所有修改事件的操作都必须先调它。 */
  function pushHistory() {
    if (!s.chapter.value) return
    s.undoStack.value.push({
      chapterId: s.chapter.value.id,
      name: s.chapter.value.name,
      events: JSON.parse(JSON.stringify(s.chapter.value.events)),
      selected: s.selectedEvent.value,
    })
    if (s.undoStack.value.length > UNDO_LIMIT) s.undoStack.value.shift()
    // 新的改动让 redo 失效
    s.redoStack.value = []
  }

  function resetHistory() {
    s.undoStack.value = []
    s.redoStack.value = []
    s.dirty.value = false
    savePending = false
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
  }

  function undo() {
    if (!s.chapter.value || s.undoStack.value.length === 0) return
    const frame = s.undoStack.value.pop()!
    s.redoStack.value.push({
      chapterId: s.chapter.value.id,
      name: s.chapter.value.name,
      events: JSON.parse(JSON.stringify(s.chapter.value.events)),
      selected: s.selectedEvent.value,
    })
    applyFrame(frame)
  }

  function redo() {
    if (!s.chapter.value || s.redoStack.value.length === 0) return
    const frame = s.redoStack.value.pop()!
    s.undoStack.value.push({
      chapterId: s.chapter.value.id,
      name: s.chapter.value.name,
      events: JSON.parse(JSON.stringify(s.chapter.value.events)),
      selected: s.selectedEvent.value,
    })
    applyFrame(frame)
  }

  function applyFrame(frame: UndoFrame) {
    if (!s.chapter.value || frame.chapterId !== s.chapter.value.id) return
    s.chapter.value.name = frame.name
    s.chapter.value.events = frame.events
    s.selectedEvent.value = Math.min(frame.selected, Math.max(0, frame.events.length - 1))
    markDirty()
  }

  /** 新建一个符合 schema 的空事件骨架 */
  function blankEvent(typeKey: string): ScriptEventData {
    const spec = g.eventSpecs.value[typeKey]
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
  }

  /**
   * 插入事件。
   *
   * 默认插到**最后一条 chapter_end 之前**而不是数组末尾 —— 新章节自带一条
   * chapter_end，插到它后面每次都会立刻报「章节结束之后还有事件，永远不会执行」，
   * 作者得先看到一条警告再手动往上挪。
   */
  function insertEvent(typeKey: string, at?: number) {
    if (!s.chapter.value) return
    pushHistory()
    const events = s.chapter.value.events
    const index = at ?? defaultInsertIndex()
    events.splice(index, 0, blankEvent(typeKey))
    s.selectedEvent.value = index
    markDirty()
  }

  function defaultInsertIndex(): number {
    const events = s.chapter.value?.events ?? []
    for (let i = events.length - 1; i >= 0; i--) {
      if (events[i]?.type === 'chapter_end') return i
    }
    return events.length
  }

  function removeEvent(index: number) {
    if (!s.chapter.value) return
    pushHistory()
    s.chapter.value.events.splice(index, 1)
    s.selectedEvent.value = Math.max(0, Math.min(index, s.chapter.value.events.length - 1))
    markDirty()
  }

  function duplicateEvent(index: number) {
    if (!s.chapter.value) return
    const src = s.chapter.value.events[index]
    if (!src) return
    pushHistory()
    s.chapter.value.events.splice(index + 1, 0, JSON.parse(JSON.stringify(src)))
    s.selectedEvent.value = index + 1
    markDirty()
  }

  function moveEvent(from: number, to: number) {
    if (!s.chapter.value) return
    if (from === to || from < 0 || to < 0) return
    if (from >= s.chapter.value.events.length || to >= s.chapter.value.events.length) return
    pushHistory()
    const [ev] = s.chapter.value.events.splice(from, 1)
    s.chapter.value.events.splice(to, 0, ev)
    s.selectedEvent.value = to
    markDirty()
  }

  /**
   * 整段移动。时间轴把「转场」「AI 互动轮次」折叠成一行，拖那一行时移动的
   * 是整块而不是一条 —— 拆开搬会把这几条事件打散，那正是折叠想避免的。
   *
   * `dest` 是移动前的下标语义：先抠出来再插入会让 dest 往前挪，所以这里
   * 显式把「往后搬」的情况减掉抠走的长度。
   */
  function moveEventRange(from: number, count: number, dest: number) {
    if (!s.chapter.value) return
    const list = s.chapter.value.events
    if (count < 1 || from < 0 || from + count > list.length) return
    if (dest >= from && dest < from + count) return // 落回自己身上
    pushHistory()
    const block = list.splice(from, count)
    const at = dest > from ? dest - count : dest
    list.splice(at, 0, ...block)
    s.selectedEvent.value = at
    markDirty()
  }

  /** 改事件的一个字段。空值一律删键，避免往 YAML 里写一堆空字符串。 */
  function setEventField(index: number, key: string, value: unknown) {
    if (!s.chapter.value) return
    const ev = s.chapter.value.events[index]
    if (!ev) return
    pushHistory()
    if (value === '' || value === null || value === undefined) delete ev[key]
    else ev[key] = value
    markDirty()
  }

  /** 整体替换一个事件（换类型时用） */
  function replaceEvent(index: number, next: ScriptEventData) {
    if (!s.chapter.value) return
    pushHistory()
    s.chapter.value.events[index] = next
    markDirty()
  }

  function setChapterName(name: string) {
    if (!s.chapter.value) return
    pushHistory()
    s.chapter.value.name = name.trim() === '' ? undefined : name
    markDirty()
  }

  // ========================================================
  // 保存
  // ========================================================

  function markDirty() {
    s.dirty.value = true
    revision++
    if (saveTimer) clearTimeout(saveTimer)
    // 防抖直写正式文件。Rust 侧是原子写 + .bak，配合撤销栈兜底反悔。
    saveTimer = setTimeout(() => {
      void save()
    }, AUTOSAVE_DELAY)
    scheduleValidation()
  }

  async function save() {
    const key = g.scriptKey.value
    if (!key || !s.chapter.value) return
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
    // 已经有一次落盘在飞：记下来，等它结束后再写一次，而不是直接丢掉
    if (s.saving.value) {
      savePending = true
      return
    }

    s.saving.value = true
    const rev = revision
    try {
      await api.writeChapter({
        key,
        chapterId: s.chapter.value.id,
        name: s.chapter.value.name,
        events: s.chapter.value.events,
        extra: s.chapter.value.extra,
      })
      // 只有期间没有新改动才算干净 —— 否则那次编辑还没落盘
      if (rev === revision) s.dirty.value = false
      s.lastSavedAt.value = Date.now()
      syncChapterSummary()
    } catch (e) {
      notifyError('自动保存失败，改动还在编辑器里', e)
    } finally {
      s.saving.value = false
      if (savePending) {
        savePending = false
        void save()
      }
    }
  }

  /** 把当前章节的事件数/显示名同步回流程图用的摘要 */
  function syncChapterSummary() {
    if (!s.chapter.value || !s.detail.value) return
    const cs = s.detail.value.chapters.find((c) => c.id === s.chapter.value!.id)
    if (cs) {
      cs.eventCount = s.chapter.value.events.length
      cs.name = s.chapter.value.name
    }
  }

  /**
   * 把待写入的改动立刻落盘。
   *
   * 名字刻意不叫 confirmDiscard*：自动保存的语义下「丢弃」不该是一个选项，
   * 这里从不询问用户。
   */
  async function flushPendingSave(): Promise<void> {
    if (!s.dirty.value && !savePending) return
    await save()
  }

  // ========================================================
  // 章节增删改与重排
  // ========================================================

  async function createChapter(chapterId: string, name: string) {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    try {
      const created = await api.createChapter(key, chapterId, name)
      s.detail.value.chapters.push({
        id: created.id,
        name: created.name,
        group: created.id.includes('/')
          ? created.id.slice(0, created.id.lastIndexOf('/'))
          : undefined,
        eventCount: created.events.length,
      })
      s.detail.value.chapters.sort((a, b) => a.id.localeCompare(b.id))
      await runValidation()
      notifyOk('章节已创建', created.id)
    } catch (e) {
      notifyError('创建章节失败', e)
    }
  }

  async function deleteChapter(chapterId: string) {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    const dialog = useDialogStore()
    const ok = await dialog.confirm(
      `确定删除章节「${chapterId}」吗？\n\n文件将被永久删除，指向它的跳转会断链。`,
      '删除章节',
    )
    if (!ok) return
    try {
      await api.deleteChapter(key, chapterId)
      s.detail.value.chapters = s.detail.value.chapters.filter((c) => c.id !== chapterId)
      if (s.chapter.value?.id === chapterId) {
        s.chapter.value = null
        s.level.value = 'flow'
        resetHistory()
      }
      await runValidation()
    } catch (e) {
      notifyError('删除章节失败', e)
    }
  }

  /**
   * 重排一条 linear 链。
   *
   * 章节先后是 chapter_end.next_chapter 串出来的，所以这里做的是重新接线，
   * 不是改文件名顺序。分支章节会被后端拒绝。
   */
  // ========================================================
  // 校验
  // ========================================================

  /**
   * 校验要扫全部剧本（为了查剧本名重复）再逐章读盘，比保存重得多，
   * 所以用比自动保存更长的防抖，而不是每次落盘都跟着跑一遍。
   */
  function scheduleValidation() {
    if (validateTimer) clearTimeout(validateTimer)
    validateTimer = setTimeout(() => {
      void runValidation()
    }, VALIDATE_DELAY)
  }

  async function runValidation() {
    const key = g.scriptKey.value
    if (!key) return
    if (validateTimer) {
      clearTimeout(validateTimer)
      validateTimer = null
    }
    const seq = ++validateSeq
    try {
      const report = await api.validateScript(key)
      if (seq === validateSeq) s.report.value = report
    } catch (e) {
      if (seq === validateSeq) notifyError('校验失败', e)
    }
  }

  // ========================================================
  // 试玩
  // ========================================================

  /**
   * 在编辑器内试玩。
   *
   * 保存不拦 error，试玩拦 —— 跑一个已知跑不通的剧本只会浪费作者时间。
   * `fromChapter` 留空则从开场章节开始。
   */
  async function startPreview(fromChapter?: string): Promise<boolean> {
    const key = g.scriptKey.value
    if (!key) return false
    // 剧本自然跑完后 previewing 仍是 true（试玩界面停留在终场），此时再点试玩
    // 不会触发 PreviewStage 的 watch（true→true），上一轮的立绘/台词会直接带进
    // 新一轮。先停掉一轮，让 watch 完整走一遍 快照/清理/还原 再开新场。
    if (s.previewing.value) {
      await stopPreview()
    }
    await flushPendingSave()
    await runValidation()
    if (g.hasBlockingErrors.value) {
      s.tab.value = 'validate'
      notifyWarn(
        '还有问题没解决',
        `${s.report.value?.errorCount} 个错误会让剧本跑不通，先修掉再试玩`,
      )
      return false
    }
    // 主角定不下来的话，第一个 character: MAIN 事件就会把剧本卡死在原地。
    // 与其让作者对着不动的画面猜，不如现在就说清楚。
    await checkReadiness()
    if (s.readiness.value && !s.readiness.value.ok) {
      notifyWarn('试玩前还差一步', s.readiness.value.reason ?? '主角未确定')
      return false
    }
    try {
      const info = await api.startPreview(key, fromChapter)
      s.previewing.value = true
      s.previewGeneration.value = info.generation
      await refreshScripts()
      return true
    } catch (e) {
      notifyError('试玩启动失败', e)
      return false
    }
  }

  /** 查一次试玩可行性。失败不算错误 —— 引擎没起来时也不该拦着人编辑 */
  async function checkReadiness() {
    const key = g.scriptKey.value
    if (!key) return
    try {
      s.readiness.value = await api.previewReadiness(key)
    } catch (e) {
      console.warn('试玩可行性检查失败:', e)
      s.readiness.value = null
    }
  }

  async function stopPreview() {
    if (!s.previewing.value) return
    s.previewing.value = false
    s.previewGeneration.value = null
    try {
      await api.stopPreview()
    } catch (e) {
      console.warn('停止试玩失败:', e)
    }
    // 关键：必须在后端 stopPreview 返回之后再清一次。
    // PreviewStage 的 watch 在 previewing=false 时已经 clear 过一次，但那次早于
    // 后端试玩任务收尾——任务在 await 期间还会继续 emit 晚到的占位/旁白事件，
    // 它们入队（队列已暂停不处理），等下次进自由对话 resume 时被消费，就串到
    // 正常对话的首句（issue #2）。这里把晚到事件排空。
    eventQueue.clear()
  }

  // ========================================================
  // 素材 / 角色
  // ========================================================

  /**
   * 导入素材。`scope` 决定落点：剧本独有（随剧本分发）或全局（所有剧本共享）。
   * 返回落盘后的文件名 —— Rust 会做一次名称清洗，可能与源文件名不同。
   */
  async function uploadAsset(
    kind: AssetKind,
    scope: AssetScope,
    srcPath: string,
  ): Promise<string | null> {
    const key = g.scriptKey.value
    if (!key) return null
    try {
      const saved = await api.uploadAsset(key, kind, scope, srcPath)
      if (scope === 'global') {
        await refreshGlobalAssets()
      } else if (s.detail.value) {
        s.detail.value.assets[kind].push(saved)
        s.detail.value.assets[kind].sort()
      }
      // 素材页开着的时候要跟着变，否则刚导进来的图不出现在列表里
      if (s.assetFiles.value.script || s.assetFiles.value.global) void refreshAssetFiles()
      notifyOk(
        scope === 'global' ? '已导入为全局素材' : '已导入为剧本素材',
        saved,
      )
      return saved
    } catch (e) {
      notifyError('导入素材失败', e)
      return null
    }
  }

  async function createCharacter(folder: string, aiName: string, systemPrompt: string) {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    try {
      const c = await api.createCharacter(key, folder, aiName, systemPrompt)
      s.detail.value.characters.push(c)
      void refreshGlobalCharacters()
      notifyOk('角色已创建', `剧本里写 character: ${c.roleKey}`)
    } catch (e) {
      notifyError('创建角色失败', e)
    }
  }

  /**
   * 删除剧本内一个角色。
   * 删完后跑一次校验——剧本里若有 `character: <被删角色>` 的引用，会立刻变成
   * 一条看得见的诊断，提示作者哪里还在用它。
   */
  async function deleteCharacter(folder: string, displayName: string) {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    const dialog = useDialogStore()
    const ok = await dialog.confirm(
      `确定删除角色「${displayName}」吗？\n\n` +
        '整个目录（人设、立绘）将被永久删除，无法恢复。' +
        '仍在引用它的台词会显示为校验错误。',
      '删除角色',
    )
    if (!ok) return
    try {
      await api.deleteCharacter(key, folder)
      s.detail.value.characters = s.detail.value.characters.filter(
        (c) => c.folder !== folder,
      )
      void refreshGlobalCharacters()
      await runValidation()
      notifyOk('角色已删除', `${displayName} 已被永久删除`)
    } catch (e) {
      notifyError('删除角色失败', e)
    }
  }

  /** 素材页的详细列表。两次调用（剧本 + 全局），页面打开时拉一次 */
  async function refreshAssetFiles() {
    const key = g.scriptKey.value
    if (!key) return
    try {
      const [script, global] = await Promise.all([
        api.listAssetFiles(key, 'script'),
        api.listAssetFiles(key, 'global'),
      ])
      s.assetFiles.value = { script, global }
    } catch (e) {
      console.warn('读取素材详情失败:', e)
    }
  }

  async function deleteAsset(kind: AssetKind, scope: AssetScope, name: string) {
    const key = g.scriptKey.value
    if (!key) return
    const dialog = useDialogStore()
    const where =
      scope === 'global'
        ? '（全局素材）删掉之后所有引用它的剧本都会找不到文件。'
        : '（本剧本素材）'
    const ok = await dialog.confirm(
      `确定删除「${name}」吗？\n\n${where}\n如果还有事件在引用它，删完校验会报「素材找不到」。`,
      '删除素材',
    )
    if (!ok) return
    try {
      await api.deleteAsset(key, kind, scope, name)
      // 三份列表都要刷：素材页的详情、属性面板下拉用的剧本内清单与全局清单
      await Promise.all([
        refreshAssetFiles(),
        refreshGlobalAssets(),
        reloadDetailAssets(),
      ])
      await runValidation()
      notifyOk('素材已删除', `${name} 已被永久删除`)
    } catch (e) {
      notifyError('删除素材失败', e)
    }
  }

  /** 只把 detail 里的素材清单刷新一下，不动章节与编辑状态 */
  async function reloadDetailAssets() {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    try {
      const fresh = await api.readScript(key)
      s.detail.value.assets = fresh.assets
    } catch (e) {
      console.warn('刷新剧本素材清单失败:', e)
    }
  }

  async function refreshGlobalCharacters() {
    const key = g.scriptKey.value
    if (!key) return
    try {
      s.globalCharacters.value = await api.listGlobalCharacters(key)
    } catch (e) {
      console.warn('读取全局角色失败:', e)
    }
  }

  /** 拉取全局成就列表（id → 标题），供成就下拉选项；失败静默，下拉会变空 */
  async function loadAchievements() {
    try {
      const list = await achievementApi.getAchievementList()
      s.achievements.value = Object.fromEntries(
        Object.entries(list).map(([id, a]) => [id, a.title ?? id]),
      )
    } catch (e) {
      console.warn('读取成就列表失败:', e)
    }
  }

  /**
   * 从全局角色库导入一个角色。
   *
   * 复制而不是引用：引擎解析 `character:` 只在剧本自己的 characters/ 里找
   * （见 script_function::get_role），全局角色库不在那条路径上。所以「直接选
   * 用全局角色」在引擎层面做不到，能做到的是「别让作者重新敲一遍人设」。
   */
  async function importGlobalCharacter(folder: string, withAvatar: boolean) {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    try {
      const c = await api.importGlobalCharacter(key, folder, withAvatar)
      s.detail.value.characters.push(c)
      s.detail.value.characters.sort((a, b) => a.folder.localeCompare(b.folder))
      await refreshGlobalCharacters()
      notifyOk(
        `已导入角色「${c.aiName}」`,
        withAvatar
          ? `立绘也复制了一份，剧本可以单独分发。剧本里写 character: ${c.roleKey}`
          : `立绘仍读全局那份。剧本里写 character: ${c.roleKey}`,
      )
    } catch (e) {
      notifyError('导入角色失败', e)
    }
  }

  // ========================================================
  // 剧本设置
  // ========================================================

  async function saveStoryConfig(config: Record<string, unknown>) {
    const key = g.scriptKey.value
    if (!key || !s.detail.value) return
    try {
      await api.writeStoryConfig(key, config)
      s.detail.value.storyConfig = config
      await refreshScripts()
      await runValidation()
      notifyOk('剧本设置已保存')
    } catch (e) {
      notifyError('保存剧本设置失败', e)
    }
  }

  // ========================================================
  // 提示
  // ========================================================

  function notifyOk(title: string, message = '') {
    // skipTipsCheck 必传：没有 tips.txt 时 showNotification 会静默吞掉一切
    useUIStore().showNotification({ type: 'success', title, message, skipTipsCheck: true })
  }
  function notifyWarn(title: string, message = '') {
    useUIStore().showNotification({ type: 'warning', title, message, skipTipsCheck: true })
  }
  function notifyError(title: string, err: unknown) {
    useUIStore().showNotification({
      type: 'error',
      title,
      message: typeof err === 'string' ? err : String(err),
      skipTipsCheck: true,
      duration: 6000,
    })
  }

  return {
    init,
    refreshScripts,
    refreshGlobalAssets,
    openScript,
    closeScript,
    deleteScript,
    syncEngine,
    openChapter,
    backToFlow,
    undo,
    redo,
    blankEvent,
    insertEvent,
    removeEvent,
    duplicateEvent,
    moveEvent,
    moveEventRange,
    setEventField,
    replaceEvent,
    setChapterName,
    save,
    flushPendingSave,
    createChapter,
    deleteChapter,
    runValidation,
    startPreview,
    stopPreview,
    uploadAsset,
    createCharacter,
    deleteCharacter,
    refreshAssetFiles,
    deleteAsset,
    refreshGlobalCharacters,
    loadAchievements,
    importGlobalCharacter,
    saveStoryConfig,
    notifyOk,
    notifyWarn,
    notifyError,
  }
}
