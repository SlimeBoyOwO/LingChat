/**
 * 压缩包导入的前端公共流程。
 *
 * 角色与插件的导入在 UI 层面完全同构（选文件 → 后端解压 → 进度条 → 取消 → 结果提示），
 * 差异只在三处：事件名前缀、状态所在 store、后端命令。这里把公共部分抽成工厂，
 * 各领域用 `createArchiveImportController` 绑定自己的三元组，
 * 再在模块作用域调用一次以获得共享的监听器与 task_id（与原 `useRoleImportExport` 一致）。
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { decodePathFileName } from '@/utils/path'
import type {
  ArchiveFormat,
  ArchiveImportResult,
  EntryEvent,
  ImportStartedEvent,
} from '@/api/services/archive'
import type { ArchiveImportSlice } from '@/stores/modules/ui/archive-import'

export type { ArchiveImportSlice, ImportPhase } from '@/stores/modules/ui/archive-import'

export interface ArchiveImportStoreLike {
  import: ArchiveImportSlice
  resetImport(): void
}

export interface ArchiveImportBinding {
  /** 事件前缀：`role` → `role:import-started` / `role:import-progress`。 */
  eventPrefix: string
  getStore: () => ArchiveImportStoreLike
  invoke: (args: {
    path: string
    fileName: string
    format?: ArchiveFormat
    conflict: string
  }) => Promise<ArchiveImportResult>
  cancelTask: (taskId: string) => Promise<void>
  successMessage: (result: ArchiveImportResult) => string
  defaultConflict: string
  /** 后端额外发送纯文本错误的事件名（角色侧存在但当前后端未发送，保留兼容）。 */
  errorEvent?: string
  logTag?: string
}

// 名称超过 28 个字符时从左侧截断，并添加省略号。
function truncateName(name: string, max = 28): string {
  if (name.length <= max) return name
  return '…' + name.slice(name.length - max + 1)
}

export function detectArchiveFormat(fileName: string): ArchiveFormat | null {
  const lower = fileName.toLowerCase()
  if (lower.endsWith('.zip')) return 'zip'
  if (lower.endsWith('.7z')) return '7z'
  return null
}

function isAndroidContentUri(p: string): boolean {
  return p.startsWith('content://')
}

export function createArchiveImportController(binding: ArchiveImportBinding) {
  const tag = binding.logTag ?? 'ArchiveImport'
  let progressUnlisten: UnlistenFn | null = null
  let errorUnlisten: UnlistenFn | null = null
  let startedUnlisten: UnlistenFn | null = null
  let progressTimer: number | null = null
  let listenersInitialized = false
  // 当前正在进行的导入任务 id；cancel() 时传给后端以找到正确的取消令牌。
  // 后端有全局导入并发锁，所以同一时刻最多只有一个任务。
  let currentTaskId: string | null = null
  // 用户在进度条上主动点过取消。后端 invoke 的 Err 会在稍后才返回，用它区分
  // 「取消信号」与「真实失败」，避免把「已取消」状态覆盖成报错。
  let cancelledByUser = false

  function clearTimers() {
    if (progressTimer !== null) {
      window.clearInterval(progressTimer)
      progressTimer = null
    }
  }

  const store = () => binding.getStore()

  async function ensureListeners() {
    if (listenersInitialized) return
    listenersInitialized = true
    // 只缓存 store 实例：`resetImport()` 会整体替换 `import` 对象，
    // 提前取出 `store.import` 会导致监听器写入一个已被丢弃的旧对象。
    const st = store()
    progressUnlisten = await listen<EntryEvent>(`${binding.eventPrefix}:import-progress`, (e) => {
      const evt = e.payload
      if (evt.phase === 'entry') {
        if (evt.bytes_total > 0) {
          const pct = Math.min(90, Math.floor((evt.bytes_done / evt.bytes_total) * 90))
          st.import.percent = pct
        }
        st.import.message = truncateName(evt.name)
      } else if (evt.phase === 'finished') {
        st.import.percent = 100
      }
    })
    startedUnlisten = await listen<ImportStartedEvent>(
      `${binding.eventPrefix}:import-started`,
      (e) => {
        // 后端刚生成 task_id 时立刻发送，前端存下来给 cancel() 用。
        currentTaskId = e.payload?.task_id ?? null
      },
    )
    if (binding.errorEvent) {
      const errorEvent = binding.errorEvent
      errorUnlisten = await listen<string>(errorEvent, (e) => {
        if (cancelledByUser) return // 用户已主动取消，不覆盖「已取消」状态
        store().import.phase = 'error'
        store().import.error = e.payload || 'import failed'
        clearTimers()
      })
    }
  }

  // 使用基于耗时的指数曲线模拟进度，最高推进到 90%。
  function startFakeProgress() {
    store().import.percent = 0
    const start = Date.now()
    clearTimers()
    progressTimer = window.setInterval(() => {
      const elapsed = Date.now() - start
      const pct = Math.min(90, Math.floor(90 * (1 - Math.exp(-elapsed / 3000))))
      store().import.percent = pct
      if (pct >= 90) {
        store().import.message = '完成中'
      }
    }, 200)
  }

  async function pickAndImport(conflict: string = binding.defaultConflict) {
    console.log(`[${tag}] pickAndImport 开始, conflict=`, conflict)
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'Archive', extensions: ['zip', '7z'] }],
    })
    if (!selected) {
      console.log(`[${tag}] pickAndImport 用户取消选择`)
      return
    }
    const filePath = typeof selected === 'string' ? selected : (selected as any).path
    if (!filePath) return
    // content:// URI 末段是 URL 编码的（如 `E6A998E585891784606515054.zip`），
    // 必须 decode 后才能得到真实文件名（`诺一铃灵.zip`）。
    const fileName = decodePathFileName(filePath) || filePath
    // 扩展名缺失时不再硬报错；后端用 magic 决定真实格式。
    const format: ArchiveFormat | undefined = detectArchiveFormat(fileName) ?? undefined
    await runImport(filePath, fileName, format, conflict)
  }

  async function runImport(
    filePath: string,
    fileName: string,
    format: ArchiveFormat | undefined,
    conflict: string,
  ) {
    store().resetImport()
    store().import.phase = 'running'
    store().import.fileName = truncateName(fileName)
    // hint 可能为 undefined；用 'zip' 占位，等 result 返回后用真实格式覆盖。
    store().import.format = format ?? 'zip'
    store().import.conflict = conflict
    store().import.startedAt = Date.now()
    store().import.percent = -1
    // 每次都先把 task_id 清空，等后端 emit `<prefix>:import-started` 后再回填。
    currentTaskId = null
    cancelledByUser = false
    await ensureListeners()

    try {
      console.log(
        `[${tag}] backend path import: source=%s, androidSaf=%s`,
        filePath,
        isAndroidContentUri(filePath),
      )
      startFakeProgress()
      const result = await binding.invoke({ path: filePath, fileName, format, conflict })

      store().import.result = result
      store().import.format = result.format // 后端 magic 决定的真实格式
      store().import.phase = 'done'
      store().import.percent = 100
      store().import.message = binding.successMessage(result)
      console.log(
        `[${tag}] runImport 完成: action=%s, bytes=%d`,
        result.conflict_action,
        result.bytes_extracted,
      )
    } catch (e: any) {
      console.error(`[${tag}] runImport 失败:`, e)
      if (cancelledByUser) {
        // 用户主动取消后，后端 invoke 的 Err 只是取消信号，不是真实失败。
        store().import.phase = 'cancelled'
        store().import.message = '已取消'
      } else {
        store().import.phase = 'error'
        store().import.error = typeof e === 'string' ? e : e?.message || String(e)
      }
    } finally {
      currentTaskId = null
      clearTimers()
    }
  }

  async function cancel() {
    console.log(`[${tag}] cancel 发送取消请求`, { taskId: currentTaskId })
    // 先置位再发请求：后端 invoke 的 Err 可能在 await 期间就返回，
    // 置位必须早于它，catch 才能正确识别为「取消」而非「失败」。
    cancelledByUser = true
    if (!currentTaskId) {
      // 极端情况：用户在 task_id 还没回填时（或者根本没在导入）就点了取消。
      console.warn(`[${tag}] cancel 没有可用的 task_id，跳过后端调用`)
    } else {
      try {
        await binding.cancelTask(currentTaskId)
      } catch (e) {
        console.warn(`[${tag}] cancel 后端调用失败:`, e)
      }
    }
    store().import.phase = 'cancelled'
    store().import.message = '已取消'
    clearTimers()
  }

  return {
    setupListeners: ensureListeners,
    pickAndImport,
    runImport,
    cancel,
  }
}
