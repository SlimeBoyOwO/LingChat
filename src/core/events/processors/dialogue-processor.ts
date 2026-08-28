import type { IEventProcessor } from '../event-processor'
import type { ScriptDialogueEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useSettingsStore } from '../../../stores/modules/settings'
import { useFusedStore } from '../../../stores/modules/ui/fused'
import { isJaLocale, hkify } from '@/locales'

export default class DialogueProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'reply'
  }

  async processEvent(event: ScriptDialogueEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()
    const settingsStore = useSettingsStore()
    const fused = useFusedStore()

    // 更新游戏状态显示对话
    gameStore.currentStatus = 'responding'

    // 针对剧本模式，获取角色
    const role = await gameStore.getOrCreateGameRole(event.roleId)
    if (!role) {
      console.warn('角色修改的角色似乎并没有被初始化')
      return
    }

    const displayName = event.displayName ? event.displayName : role.roleName
    const displaySubtitle = event.displaySubtitle ? event.displaySubtitle : role.roleSubTitle

    // 日文界面且存在日语译文时显示日语译文；繁体（香港）界面下对话转繁体显示
    const displayLine = hkify(isJaLocale() && event.ttsText ? event.ttsText : event.message || '')
    gameStore.currentLine = displayLine
    uiStore.showCharacterMotionText = event.motionText || ''

    gameStore.appendGameMessage({
      type: 'reply',
      displayName: displayName,
      content: event.message,
      emotion: event.emotion,
      audioFile: event.audioFile,
      isFinal: event.isFinal,
      motionText: event.motionText,
      originalTag: event.originalTag,
      userMessageSeq: event.userMessageSeq,
      thinking: event.thinking,
      ttsText: event.ttsText,
      senderRoleId: event.roleId,
    })

    // 回溯更新最近一条没有序号标记的用户消息（前端发送消息时尚未拿到序号）
    if (typeof event.userMessageSeq === 'number') {
      const history = gameStore.dialogHistory
      for (let i = history.length - 1; i >= 0; i--) {
        if (history[i].type === 'message' && history[i].userMessageSeq === undefined) {
          history[i].userMessageSeq = event.userMessageSeq
          break
        }
      }
    }

    // ── 台词融合:判断本条是否并入当前台词(pending 队列) ──
    // 首段/角色切换 → showCharacterLine(启动打字机/开新台词框);
    // 同角色后续段 → 一律进 pending 由打字机按序消费
    // (合并段 merge=true 追加,独立段 merge=false 开新台词),
    // 保证不丢内容、不乱序。
    // 通过 event.duration=0 跳过队列等待(event-queue 零改动)。
    //
    // 合并判定不依赖实时媒体状态(isTyping/audioFinished 异步同步,处理器同步
    // 执行时不可靠),改用 interrupted latch:用户点击 → markInterrupted →
    // 紧接着下一条段独立成句(等价"媒体已完成"),判定处 consumeInterrupted
    // 消费(一次性)后后续段恢复合并 —— 点击只是快进,不退出融合。
    // 跨角色/新回合首段在 !merged 分支同样消费,避免上回合落定点击
    // 残留的 latch 把整个新回合打成逐句(多角色融合失效的根因)。
    const fusedEnabled = settingsStore.text.fusedDialogue && !gameStore.runningScript
    const textLen = (event.message || '').length + (event.motionText || '').length
    const threshold = settingsStore.text.fusedThreshold

    let merged = false

    if (fusedEnabled && event.roleId === fused.lastSenderRoleId) {
      // 同角色:后续段统一进 pending
      if (event.isFinal) {
        // isFinal:并入作末段(merge=true 追加),保持 duration=-1 等点击落定。
        // 判定不只看 pendingCount(pending 由打字机/音频异步消费,
        // 安卓端节奏快时
        // 上一段可能已被打完而队列已空)——最近一段是合并段即视为
        // 融合仍在进行。
        if (fused.pendingCount > 0 || fused.lastSegmentMerged) {
          fused.pushPending({
            text: displayLine,
            motionText: event.motionText || '',
            emotion: event.emotion || '',
            originalTag: event.originalTag || '',
            audioFile: event.audioFile,
            userMessageSeq: event.userMessageSeq,
            roleId: event.roleId,
            merge: true,
          })
          fused.accumulate(event.roleId, textLen)
          merged = true
        }
      } else if (
        !fused.interrupted &&
        textLen <= threshold &&
        fused.accumulatedTextLength + textLen <= threshold
      ) {
        // 未被打断 + 字数达标 → 合并,追加当前台词
        fused.pushPending({
          text: displayLine,
          motionText: event.motionText || '',
          emotion: event.emotion || '',
          originalTag: event.originalTag || '',
          audioFile: event.audioFile,
          userMessageSeq: event.userMessageSeq,
          roleId: event.roleId,
          merge: true,
        })
        fused.accumulate(event.roleId, textLen)
        event.duration = 0 // 跳过队列等待,立即处理下一条
        merged = true
      } else {
        // 独立段(点击打断/超长/累积超限):进 pending 作新台词起点
        fused.pushPending({
          text: displayLine,
          motionText: event.motionText || '',
          emotion: event.emotion || '',
          originalTag: event.originalTag || '',
          audioFile: event.audioFile,
          userMessageSeq: event.userMessageSeq,
          roleId: event.roleId,
          merge: false,
        })
        fused.resetAccumulation(event.roleId, textLen)
        // 消费打断 latch:本条独立成句后,后续段恢复合并(点击只是快进)
        fused.consumeInterrupted()
        event.duration = 0
        merged = true
      }
    }

    if (!merged) {
      // 首段/角色切换/开关关:正常展示 + 重置累积基准
      // (独立句 = 新台词起点)
      // 消费打断 latch:跨角色/新回合首段天然独立,不该继承上次
      // 点击的打断状态
      // (否则上一回合落定点击会让整个新回合逐句,多角色时融合失效)
      fused.consumeInterrupted()
      // 丢弃 pending 里上一角色的未消费段(历史已存,仅弃展示)——
      // 否则旧角色剩余段会在新角色台词播放中被续打出来(内容/名字错乱)
      fused.discardPending()
      // 新台词起点:isFinal 不再并入上一段
      fused.resetLastSegment()
      uiStore.showCharacterLine = gameStore.currentLine
      fused.resetAccumulation(event.roleId, textLen)
    }

    // 融合激活:非 isFinal 段跳过队列等待 —— 合并分支上面已置 duration=0,
    // 此处兜底 !merged 的首段/角色切换首句:否则 queue 停在首段等点击,
    // 后续段无法入队(pending 永远填不进下一段)。
    // isFinal 保持 duration=-1 等点击落定本轮。
    if (fusedEnabled && !event.isFinal) {
      event.duration = 0
    }

    // 情感/表情/标题:每段照常更新(合并段也切换)
    // 合并段不设置 role.emotion/showCharacterEmotion —— 多条段快速入队时
    // 情绪/立绘会在打字机播放前瞬间跳变,而非随段落逐一切换;
    // 改由 GameDialog/DialogueBox 的 continueFused 消费段时切换。
    gameStore.currentInteractRoleId = role.roleId
    if (!merged) {
      role.emotion = event.emotion || '正常'
      role.originalEmotion = event.originalTag || '正常'
      // 合并段不立即切换音频(会打断当前段语音),由 continueFused 续打时切换
      uiStore.currentAvatarAudio = event.audioFile || 'None'
      uiStore.showCharacterEmotion = role.originalEmotion
    }

    uiStore.showCharacterTitle = displayName
    uiStore.showCharacterSubtitle = displaySubtitle
    // gameStore.currentCharacter = event.character;

    // 对话总是等待用户继续，所以这里不需要做任何等待
    // event-queue 会自动检测到这是对话事件并等待用户继续
  }
}
