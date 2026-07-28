/**
 * 把 WebView 聊天状态持续推送给 Android 悬浮桌宠 Service（仅 Android 生效）。
 *
 * - 监听 game.currentLine / currentInteractRoleId / currentStatus，
 *   生成 PetStatePayload 并通过节流 pusher 推送到 updatePetState。
 * - 监听 settings.pet.scale / settings.audio.characterVolume 同步缩放与音量。
 * - 监听 settings.floatingPet.autoShowOnLaunch：第一次出现已启用时，
 *   把 store.enabled 打开并调 store.activate() 让 Service 自启。
 * - 在 App.vue 挂载一次即可。卸载时清掉所有 watcher / 事件订阅。
 */
import { onMounted, onBeforeUnmount, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useSettingsStore } from '@/stores/modules/settings'
import { useFloatingPetStore, useFloatingPetPusher } from '@/stores/modules/floating-pet'
import { isAndroid } from '@/utils/platform'

/** 把 GameRole 拍平为 PetStatePayload.character。avatar URL 用现有约定路径。 */
function toCharacterPayload(
  roleId: number,
  role: {
    roleName: string
    emotion: string
    originalEmotion: string
    character_folder: string
  },
) {
  const folder = role.character_folder
  // 与 Notification.vue / GameDialog 已有约定保持一致:
  // /characters/<folder>/头像.png — Tauri webview 可解析相对路径，
  // Kotlin 端按需要再决定是否要 resource:// 本地解码。
  return {
    id: String(roleId),
    name: role.roleName,
    avatarUrl: `/characters/${folder}/头像.png`,
    expression: role.emotion || role.originalEmotion || 'default',
  }
}

export function useFloatingPetBridge() {
  // 仅 Android 端需要持续推流；桌面/iOS 直接短路，省掉节流里每次的 invoke 调用
  if (!isAndroid()) return

  const game = useGameStore()
  const settings = useSettingsStore()
  const pet = useFloatingPetStore()
  const push = useFloatingPetPusher()

  // 把 enabled 同步给 store：SettingsFloatingPet.vue 改的其实是 settings.floatingPet.enabled,
  // 这里桥接到 store.enabled 这样 store.activate() 才能正确判别。
  const syncEnabled = () => {
    pet.setEnabled(!!settings.floatingPet?.enabled)
  }
  syncEnabled()

  // ---- 1) 文本行 ----
  const stopText = watch(
    () => game.currentLine,
    (text) => {
      push({
        dialogue: {
          text: text ?? '',
          isTyping: game.currentStatus !== 'input',
          audioPlaying: false,
        },
      })
    },
    { immediate: true },
  )

  // ---- 2) 角色 ----
  const stopRole = watch(
    () => [game.currentInteractRoleId, game.currentStatus] as const,
    ([id]) => {
      if (id === null || id === undefined) {
        push({ character: undefined })
        return
      }
      const role = game.gameRoles?.[id as number]
      if (!role) return
      push({
        character: toCharacterPayload(id as number, role),
      })
    },
    { immediate: true },
  )

  // ---- 3) 缩放 / 音量 / 背景效果 ----
  const stopMeta = watch(
    () => ({
      scale: settings.pet?.scale ?? 1.0,
      volume: Math.round(settings.audio?.characterVolume ?? 100),
      backgroundEffect: settings.display?.backgroundEffect ?? 'none',
      snapToEdge: !!settings.floatingPet?.snapToEdge,
      enabled: !!settings.floatingPet?.enabled,
    }),
    (next) => {
      if (next.scale !== undefined) push({ scale: next.scale })
      if (next.volume !== undefined) push({ volume: next.volume })
      if (next.backgroundEffect !== undefined) {
        push({ backgroundEffect: next.backgroundEffect })
      }
    },
    { immediate: true },
  )

  // ---- 4) enabled 切换：开=激活，关=隐藏并清虚拟化 ----
  const stopEnabled = watch(
    () => settings.floatingPet?.enabled,
    (enabled) => {
      syncEnabled()
      if (enabled) {
        // 激活前显式再推一次 metadata (上面 watch 已是 immediate，
        // 这里再 push 一次以防启用瞬间的 race)
        push({
          scale: settings.pet?.scale ?? 1.0,
          volume: Math.round(settings.audio?.characterVolume ?? 100),
        })
      } else {
        void pet.deactivate()
      }
    },
  )

  // ---- 5) autoShowOnLaunch: 首次达到 ready 后自动 activate ----
  let autoShowFired = false
  const stopAutoShow = watch(
    () => pet.isReady,
    async (ready) => {
      if (autoShowFired) return
      if (!ready) return
      if (!settings.floatingPet?.autoShowOnLaunch) return
      autoShowFired = true
      await pet.activate(settings.pet?.scale ?? 1.0)
    },
    { immediate: true },
  )

  // ---- 6) 事件总线：tap / double_tap / long_press 等 ----
  let unbindEventBus: (() => void) | null = null
  onMounted(() => {
    unbindEventBus = pet.bindEventBus()
  })
  onBeforeUnmount(() => {
    stopText()
    stopRole()
    stopMeta()
    stopEnabled()
    stopAutoShow()
    unbindEventBus?.()
  })
}
