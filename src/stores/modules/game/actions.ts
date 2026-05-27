// actions.ts
import type { GameState, GameMessage, GameRole } from './state'
import { getGameInfo } from '../../../api/services/game-info'
import type { GameLineInit, WebInitData } from '../../../api/services/game-info'
import { getRoleInfo } from '../../../api/services/character'
import { useUIStore } from '../ui/ui'
import { useSettingsStore } from '../settings'
import type { SceneInfo } from '@/api/services/scene'
export const actions = {
  appendGameMessage(this: GameState, message: GameMessage) {
    this.dialogHistory.push({
      ...message,
      timestamp: Date.now(),
    })
  },

  clearDialogHistory(this: GameState) {
    this.dialogHistory = []
  },

  setGameMessages(this: GameState, messages: GameMessage[]) {
    this.dialogHistory = messages
  },

  async initializeGame(this: GameState) {
    try {
      const gameInfo = await getGameInfo()
      applyWebInitData(this, gameInfo)
      return gameInfo
    } catch (error) {
      console.error('初始化游戏信息失败:', error)
      throw error
    }
  },

  async getOrCreateGameRole(this: GameState, role_id: number): Promise<GameRole> {
    if (this.gameRoles[role_id]) {
      return this.gameRoles[role_id]
    }
    try {
      const roleInfo = await getRoleInfo(role_id)
      this.gameRoles[role_id] = {
        roleId: roleInfo.character_id,
        roleName: roleInfo.ai_name,
        roleSubTitle: roleInfo.ai_subtitle,
        thinkMessage: roleInfo.thinking_message,
        scale: roleInfo.scale,
        offsetX: roleInfo.offset_x,
        offsetY: roleInfo.offset_y,
        scaleP: roleInfo.scale_p,
        offsetXP: roleInfo.offset_x_p,
        offsetYP: roleInfo.offset_y_p,
        bubbleLeft: roleInfo.bubble_left,
        bubbleTop: roleInfo.bubble_top,
        clothes: roleInfo.clothes,
        clothesName: roleInfo.clothes_name,
        bodyPart: roleInfo.body_part,
        character_folder: roleInfo.character_folder,
        emotion: '正常',
        originalEmotion: '正常',
        show: true,
      }
      return this.gameRoles[role_id]
    } catch (error) {
      console.error('游戏角色信息获取失败:', error)
      throw error
    }
  },

  /** 标记进入剧情模式（用于控制UI显示：隐藏番茄钟/日程等） */
  enterStoryMode(this: GameState, scriptName: string = 'unknown') {
    this.runningScript = {
      scriptName,
      currentChapterName: '',
      choices: [],
      isRunning: true,
      freeDialogueInfo: {
        isFreeDialogue: false,
        maxRounds: -1,
        currentRound: 0,
        endLine: '',
      },
    }
    const uiStore = useUIStore()
    uiStore.bgMusicMode = 'loop-single'
  },

  /** 标记退出剧情模式，回到自由对话模式 */
  exitStoryMode(this: GameState) {
    this.runningScript = null
  },

  // 设置当前场景（仅更新 store，不调用 API）
  setCurrentScene(this: GameState, scene: SceneInfo | null) {
    this.currentScene = scene
  },

  // 清除场景（更新 store，API 调用由组件负责）
  clearCurrentScene(this: GameState) {
    this.currentScene = null
  },
}

/** 将 WebInitData 写入 GameState（init / 角色切换共用） */
export function applyWebInitData(state: GameState, gameInfo: WebInitData): void {
  const characterInfo = gameInfo.character_settings
  const charId = characterInfo.character_id ?? 0

  state.gameRoles = {}
  state.gameRoles[charId] = {
    roleId: charId,
    roleName: characterInfo.ai_name,
    roleSubTitle: characterInfo.ai_subtitle,
    thinkMessage: characterInfo.thinking_message,
    scale: characterInfo.scale,
    offsetX: characterInfo.offset_x,
    offsetY: characterInfo.offset_y,
    scaleP: characterInfo.scale_p,
    offsetXP: characterInfo.offset_x_p,
    offsetYP: characterInfo.offset_y_p,
    bubbleLeft: characterInfo.bubble_left,
    bubbleTop: characterInfo.bubble_top,
    clothes: characterInfo.clothes,
    clothesName: characterInfo.clothes_name,
    bodyPart: characterInfo.body_part,
    character_folder: characterInfo.character_folder,
    emotion: '正常',
    originalEmotion: '正常',
    show: true,
  }
  state.presentRoleIds = [charId]
  state.mainRoleId = charId
  state.currentInteractRoleId = gameInfo.current_interact_role_id ?? charId

  const uiStore = useUIStore()
  const settingsStore = useSettingsStore()
  state.userName = characterInfo.user_name
  state.userSubtitle = characterInfo.user_subtitle

  uiStore.showCharacterTitle = characterInfo.ai_name
  uiStore.showCharacterSubtitle = characterInfo.ai_subtitle

  if (gameInfo.background !== '') uiStore.setCurrentBackground(gameInfo.background)
  if (gameInfo.background_effect !== '') uiStore.setBackgroundEffect(gameInfo.background_effect)
  if (gameInfo.background_music !== '') uiStore.currentBackgroundMusic = gameInfo.background_music

  // 同步场景感知开关
  settingsStore.setSceneAwarenessEnabled(gameInfo.scene_awareness_enabled)

  // 恢复场景状态
  if (gameInfo.current_scene) {
    state.currentScene = gameInfo.current_scene
  }

  if (gameInfo.lines && gameInfo.lines.length > 0) {
    state.dialogHistory = convertInitLines(gameInfo.lines)
  } else {
    state.dialogHistory = []
  }
}

/** 将 Rust GameLineInit 转换为前端 GameMessage 列表 */
function convertInitLines(lines: GameLineInit[]): GameMessage[] {
  const filtered = lines.filter((line) => line.attribute !== 'system')

  return filtered.map((line, index, array) => {
    const filteredContent = line.content.replace(/\{[\s\S]*?\}/g, '').trim()

    const isLast = index === array.length - 1
    const nextLine = isLast ? null : array[index + 1]
    let isFinal = false
    if (line.attribute === 'assistant') {
      if (isLast || nextLine?.attribute === 'user') {
        isFinal = true
      }
    }

    return {
      type: (line.attribute === 'user' ? 'message' : 'reply') as 'message' | 'reply',
      displayName: line.display_name || '',
      content: filteredContent,
      emotion: line.predicted_emotion || undefined,
      audioFile: line.audio_file || undefined,
      isFinal,
      motionText: line.action_content || undefined,
      originalTag: line.original_emotion || undefined,
      timestamp: Date.now(),
    }
  })
}
