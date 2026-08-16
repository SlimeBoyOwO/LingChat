import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { eventQueue } from '../core/events/event-queue'
import type { ScriptEventType } from '../types'
import { useAdventureStore } from '../stores/modules/adventure'
import { useUIStore } from '../stores/modules/ui/ui'
import { useGameStore } from '../stores/modules/game'
import { i18n } from '@/locales'
import { useScriptEditorStore } from '../stores/modules/script-editor'
import {
  clearToolCallPreparing,
  handleToolActivity,
  handleToolCallProgress,
  interruptToolActivities,
  pushToolCallRecord,
  toolDisplayName,
  type ToolActivityEvent,
} from './services/tool-settings'
import { useDialogStore } from '../stores/modules/ui/dialog'
import type { SceneInfo } from './services/scene'

function asEvent(
  payload: unknown,
  defaults: { type: string; defaultDuration: number; isFinal?: boolean },
): ScriptEventType {
  const p = payload as Record<string, unknown>
  // 优先用引擎从 YAML 读到的 duration；没写才用各事件类型的默认值。
  // 默认值语义：-1 = 等玩家点击继续；0 = 立即继续（不等待）。
  const duration = typeof p.duration === 'number' ? p.duration : defaults.defaultDuration
  return {
    ...p,
    type: defaults.type,
    duration,
    ...(defaults.isFinal !== undefined ? { isFinal: defaults.isFinal } : {}),
  } as unknown as ScriptEventType
}

/**
 * 试玩事件的迟到丢弃。
 *
 * 试玩中止后，后端游离的流式任务（publisher/consumer）可能还会 emit 几条
 * ai:reply（如 TTS 仍在生成时的句子），它们经 IPC 到达前端时试玩可能已结束、
 * 甚至新一轮试玩已开始。这类事件必须丢弃，否则会串进自由对话历史或新一轮试玩。
 *
 * 判定规则：事件带 previewGen（试玩专用字段）时，仅当「当前在试玩 且 代号与
 * 本轮一致」才收；不带该字段的是自由对话/正式剧本回复，永远放行。
 */
function isStalePreviewReply(payload: Record<string, unknown>): boolean {
  const gen = payload.previewGen
  if (typeof gen !== 'number') return false
  const store = useScriptEditorStore()
  return !store.previewing || store.previewGeneration !== gen
}

export function initializeTauriEventListeners() {
  const currentWindow = getCurrentWindow()
  const mainWindow = currentWindow.label === 'main' ? currentWindow : null

  listen('ai:reply', (event) => {
    const payload = event.payload as Record<string, unknown>
    // 试玩中止后迟到的流式回复：直接丢弃，不放进事件队列
    if (isStalePreviewReply(payload)) return
    console.log('[Tauri] ai:reply', event.payload)
    eventQueue.addEvent(asEvent(payload, { type: 'reply', defaultDuration: -1 }))
  })

  listen('ai:thinking', (event) => {
    console.log('[Tauri] ai:thinking', event.payload)
    eventQueue.addEvent(asEvent(event.payload, { type: 'thinking', defaultDuration: 0 }))
  })

  listen('ai:thinking_progress', (event) => {
    const payload = event.payload as { thinkingLength?: number }
    console.log('[Tauri] ai:thinking_progress', payload)
    const gameStore = useGameStore()
    if (typeof payload.thinkingLength === 'number') {
      gameStore.thinkingLength = payload.thinkingLength
    }
  })

  listen('ai:error', (event) => {
    const p = event.payload as Record<string, unknown>
    console.log('[Tauri] ai:error', p)
    interruptToolActivities()
    eventQueue.addEvent({
      type: 'error',
      duration: 0,
      error_code: (p.error_code as string) ?? 'default_error',
      message: (p.detail as string) ?? '',
    } as ScriptEventType)
  })

  // 工具执行生命周期：驱动自由对话顶栏的实时状态，不写入历史记录。
  listen('ai:tool_activity', (event) => {
    const payload = event.payload as ToolActivityEvent
    handleToolActivity(payload)
  })

  // 工具调用参数流式生成进度：顶栏实时显示「正在生成…N 字」
  listen('ai:tool_call_progress', (event) => {
    handleToolCallProgress(event.payload as { tool: string; chars: number })
  })

  // 一轮 LLM 流结束：清除「正在生成」进度提示（工具被忽略的收尾轮不会再有执行事件）
  listen('ai:tool_call_progress_end', () => {
    clearToolCallPreparing()
  })

  // 工具调用结果：记入「工具调用」页面历史 + 左上角弹通知
  listen('ai:tool_call', (event) => {
    const payload = event.payload as {
      tool: string
      ok: boolean
      summary: string
      error: string | null
      arguments: string
      result: string
    }
    pushToolCallRecord({
      ...payload,
      time: new Date().toLocaleTimeString(),
    })
    const toolLabel = toolDisplayName(payload.tool)
    const uiStore = useUIStore()
    if (payload.ok) {
      uiStore.showNotification({
        type: 'success',
        title: i18n.global.t('ui.toolCalls.callSuccess'),
        message: `${toolLabel}：${payload.summary}`,
        duration: 3000,
        skipTipsCheck: true,
      })
    } else {
      uiStore.showNotification({
        type: 'warning',
        title: i18n.global.t('ui.toolCalls.callFailed'),
        message: payload.error || toolLabel,
        duration: 4000,
        skipTipsCheck: true,
      })
    }
  })

  // 审批框只在主窗口挂载；独立日志窗口等不能消费审批事件。
  // 主聊天 execute_command 审批：弹确认框，把用户决定回传给等待中的工具
  mainWindow?.listen('chat:command_approval', async (event) => {
    const payload = event.payload as {
      request_id: string
      command: string
      cwd: string
      uac: boolean
    }
    const dialogStore = useDialogStore()
    const message =
      i18n.global.t('ui.toolCalls.approvalMessage', {
        command: payload.command,
        cwd: payload.cwd || i18n.global.t('ui.toolCalls.approvalDefaultCwd'),
      }) + (payload.uac ? `\n\n${i18n.global.t('ui.toolCalls.approvalUac')}` : '')
    const approved = await dialogStore.confirm(
      message,
      i18n.global.t('ui.toolCalls.approvalTitle'),
    )
    try {
      await invoke('resolve_command_approval', { requestId: payload.request_id, approved })
    } catch (e) {
      console.warn('[Tauri] 回传命令审批结果失败（可能已超时）:', e)
    }
  })

  // execute_command 中识别到删除操作时使用独立危险确认；回传到删除审批队列。
  mainWindow?.listen('chat:command_delete_approval', async (event) => {
    const payload = event.payload as {
      request_id: string
      command: string
      cwd: string
      uac: boolean
    }
    const dialogStore = useDialogStore()
    const message =
      i18n.global.t('ui.toolCalls.commandDeleteApprovalMessage', {
        command: payload.command,
        cwd: payload.cwd || i18n.global.t('ui.toolCalls.approvalDefaultCwd'),
      }) + (payload.uac ? `\n\n${i18n.global.t('ui.toolCalls.approvalUac')}` : '')
    const approved = await dialogStore.confirm(
      message,
      i18n.global.t('ui.toolCalls.commandDeleteApprovalTitle'),
    )
    try {
      await invoke('resolve_file_delete_approval', {
        requestId: payload.request_id,
        approved,
      })
    } catch (e) {
      console.warn('[Tauri] 回传删除命令审批结果失败（可能已超时）:', e)
    }
  })

  // 主聊天 delete_file 审批：先显示后端解析并校验过的真实路径，再把决定回传给工具。
  mainWindow?.listen('chat:file_delete_approval', async (event) => {
    const payload = event.payload as {
      request_id: string
      path: string
    }
    const dialogStore = useDialogStore()
    const approved = await dialogStore.confirm(
      i18n.global.t('ui.toolCalls.fileDeleteApprovalMessage', { path: payload.path }),
      i18n.global.t('ui.toolCalls.fileDeleteApprovalTitle'),
    )
    try {
      await invoke('resolve_file_delete_approval', {
        requestId: payload.request_id,
        approved,
      })
    } catch (e) {
      console.warn('[Tauri] 回传删除审批结果失败（可能已超时）:', e)
    }
  })

  listen('status:reset', (event) => {
    console.log('[Tauri] status:reset', event.payload)
    eventQueue.addEvent(asEvent(event.payload, { type: 'status_reset', defaultDuration: 0 }))
  })

  listen('tts:cleanup', (event) => {
    const payload = event.payload as {
      deleted?: number
      orphanFiles?: number
      orphanSize?: number
    }
    console.log('[Tauri] tts:cleanup', payload)
    try {
      localStorage.setItem(
        'lingchat:last_tts_cleanup',
        JSON.stringify({
          deleted: payload.deleted ?? 0,
          orphanFiles: payload.orphanFiles ?? 0,
          orphanSize: payload.orphanSize ?? 0,
          timestamp: Date.now(),
        }),
      )
    } catch (e) {
      console.warn('[Tauri] 保存 tts:cleanup 状态到 localStorage 失败:', e)
    }
  })

  // === Adventure events ===

  listen('adventure:unlocked', (event) => {
    const payload = event.payload as any
    console.log('[Tauri] adventure:unlocked', payload)
    const adventureStore = useAdventureStore()
    if (payload?.adventure_folder) {
      adventureStore.unlockNotifications.push(payload)
    }
  })

  listen('adventure:completed', (event) => {
    const payload = event.payload as any
    console.log('[Tauri] adventure:completed', payload)
    const adventureStore = useAdventureStore()
    if (payload?.adventure_folder) {
      adventureStore.markAdventureCompleted(payload.adventure_folder)
    }
  })

  // === Auto-save events ===

  listen('save:auto-saved', async (event) => {
    const payload = event.payload as { save_id: number; title: string; timestamp: string }
    console.log('[Tauri] save:auto-saved', payload)

    // Capture screenshot for auto-save slot
    const gameStore = useGameStore()
    const screenshotPath = await gameStore.captureScreenshot()
    if (screenshotPath) {
      try {
        await invoke('save_screenshot', {
          saveId: payload.save_id,
          screenshotPath,
        })
      } catch (e) {
        console.error('[Tauri] Failed to save auto-save screenshot', e)
      }
    }

    useUIStore().showNotification({
      type: 'info',
      title: i18n.global.t('api.events.autoSave.title'),
      message: i18n.global.t('api.events.autoSave.message', { time: payload.timestamp }),
      duration: 2500,
      skipTipsCheck: true,
    })
  })

  // === Script events ===

  listen('script:narration', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'narration', defaultDuration: -1 }))
  })

  listen('script:player', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'player', defaultDuration: -1 }))
  })

  listen('script:chapter-change', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'chapter_change', defaultDuration: 0 }))
  })

  listen('script:background', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'background', defaultDuration: 0 }))
  })

  listen('script:background-effect', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'background_effect', defaultDuration: 0 }))
  })

  listen('script:music', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'music', defaultDuration: 0 }))
  })

  listen('script:sound', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'sound', defaultDuration: 0 }))
  })

  // 环境音事件（多轨并行，与BGM共存）
  listen('script:ambient', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'ambient', defaultDuration: 0 }))
  })

  listen('script:present-pic', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'present_pic', defaultDuration: -1 }))
  })

  listen('script:modify-character', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'modify_character', defaultDuration: 0 }))
  })

  listen('script:input', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'input', defaultDuration: 0 }))
  })

  listen('script:choice', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'choice', defaultDuration: 0 }))
  })

  listen('script:end', (event) => {
    console.log('[Tauri] script:end', event.payload)
    eventQueue.addEvent(
      asEvent(event.payload, { type: 'script_end', defaultDuration: 0, isFinal: true }),
    )
  })

  listen('script:free-dialogue', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'free_dialogue', defaultDuration: 0 }))
  })

  // === God Agent multi-dialogue event ===

  listen('character:switch', async (event) => {
    const payload = event.payload as { type: string; roleId: number; characterName: string }
    console.log('[Tauri] character:switch', payload)
    const gameStore = useGameStore()
    const uiStore = useUIStore()
    // 先确保角色数据已加载（立绘/名字都从这里取）
    const role = await gameStore.getOrCreateGameRole(payload.roleId)
    gameStore.currentInteractRoleId = payload.roleId
    // 新角色不在场时才替换舞台（多人场景下 God Agent 只会选在场角色，不进这分支）；
    // 用替换而非 push，避免标准模式舞台出现两个角色、桌宠不生效
    if (!gameStore.presentRoleIds.includes(payload.roleId)) {
      gameStore.presentRoleIds = [payload.roleId]
    }
    // 同步主界面/桌宠标题（对话中名字由 currentInteractRole 驱动，已覆盖）
    uiStore.showCharacterTitle = role.roleName
    uiStore.showCharacterSubtitle = role.roleSubTitle
  })

  // === LLM 场景工具事件 ===

  listen('scene:switch', (event) => {
    const payload = event.payload as { type: string; scene: SceneInfo }
    console.log('[Tauri] scene:switch', payload)
    const gameStore = useGameStore()
    const uiStore = useUIStore()
    gameStore.setCurrentScene(payload.scene)
    uiStore.setCurrentBackground(payload.scene.background ?? '')
  })

  console.log(
    '[Tauri] Event listeners initialized (ai + ai:thinking_progress + tts:cleanup + adventure + auto-save + 13 script events + character:switch + scene:switch)',
  )
}
