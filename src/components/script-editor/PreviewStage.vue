<template>
  <Teleport to="body">
    <Transition name="preview">
      <div
        v-if="store.previewing"
        class="stage"
      >
        <!--
          `main-box` 是 MainChat 里的全局类（那个 <style> 没有 scoped），这里直接
          复用而不是另写一套。它是 `flex-direction: column; justify-content: flex-end`，
          对话框才会贴在屏幕底部 —— 早先这里只是个 `position: fixed` 的空壳，
          GameDialog 作为普通块元素落在最上面，于是试玩时对话框跑到了屏幕顶部。
          复用同一个类还顺带保证：以后正式游玩的布局改了，试玩跟着一起变。
        -->
        <div class="main-box">
          <!-- 复用真实的游戏渲染层。这是当初选「复用真引擎 + 真渲染层」而不是
               另写一套预览解释器的兑现点：这四个组件读的是同一份 store，
               引擎 emit 的事件经 eventQueue 进来后，表现与正式游玩逐帧一致。 -->
          <GameBackground />
          <GameRolesStage />
          <GameExtraUI />
          <GameDialog />
        </div>

        <!-- 预览专属的顶栏，明确「这是试玩」而不是真在玩 -->
        <div class="bar">
          <span class="badge">试玩中</span>
          <span class="meta">{{ label }}</span>
          <span class="tip">试玩为调试用：不记通关、不解锁羁绊冒险。会真调 LLM（按 token 计费）</span>
          <button
            class="stop"
            title="Esc"
            @click="store.stopPreview()"
          >
            结束试玩
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import { GameBackground, GameDialog, GameRolesStage } from '@/components/game/standard'
import GameExtraUI from '@/components/game/standard/GameExtraUI.vue'
import { eventQueue } from '@/core/events/event-queue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'

const store = useScriptEditorStore()
const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

const props = defineProps<{ fromChapter?: string }>()

const label = computed(() => {
  const parts = [store.detail?.package.scriptName ?? '']
  if (props.fromChapter) parts.push(`从「${props.fromChapter}」开始`)
  // 把 MAIN 解析成了谁直接写出来 —— 羁绊剧本里演错人是最难自己看出来的一类问题
  const who = store.readiness?.mainRoleName
  if (who) parts.push(`MAIN = ${who}`)
  return parts.filter(Boolean).join(' · ')
})

/**
 * 试玩期间会被引擎改写的 gameStore 字段，进出各存/还一次。
 *
 * 后端已经把 `GameStatus` 整个备份还原了（见 `PreviewSession`），但前端这份是
 * 独立的一套状态：立绘在场名单、对话历史、剧情模式标记都只存在于浏览器里，
 * 引擎 emit 的事件经 eventQueue 直接改它。不管的话，退出编辑器回自由对话，
 * 看到的还是试玩留下的立绘和台词 —— 包括「AI 已关闭」那几条占位。
 *
 * 只存这几个字段而不是整个 `$state`：其余部分（用户名、场景配置、设置）试玩
 * 不会碰，整份深拷贝反而可能把别处刚改好的东西覆盖回去。
 */
type GameSnapshot = {
  runningScript: typeof gameStore.runningScript
  presentRoleIds: number[]
  currentInteractRoleId: number | null
  mainRoleId: number
  userName: string
  currentLine: string
  currentStatus: typeof gameStore.currentStatus
  dialogHistory: typeof gameStore.dialogHistory
  command: string | null
  /** 试玩会往角色缓存里塞剧本角色，退出时要还回原样，否则回自由对话立绘会串/消失 */
  gameRoles: typeof gameStore.gameRoles
}

let snapshot: GameSnapshot | null = null

const captureGameState = (): GameSnapshot => ({
  runningScript: gameStore.runningScript,
  presentRoleIds: [...gameStore.presentRoleIds],
  currentInteractRoleId: gameStore.currentInteractRoleId,
  mainRoleId: gameStore.mainRoleId,
  userName: gameStore.userName,
  currentLine: gameStore.currentLine,
  currentStatus: gameStore.currentStatus,
  dialogHistory: [...gameStore.dialogHistory],
  command: gameStore.command,
  gameRoles: { ...gameStore.gameRoles },
})

const restoreGameState = (s: GameSnapshot) => {
  gameStore.runningScript = s.runningScript
  gameStore.presentRoleIds = s.presentRoleIds
  gameStore.currentInteractRoleId = s.currentInteractRoleId
  gameStore.mainRoleId = s.mainRoleId
  gameStore.userName = s.userName
  gameStore.currentLine = s.currentLine
  gameStore.currentStatus = s.currentStatus
  gameStore.dialogHistory = s.dialogHistory
  gameStore.command = s.command
  gameStore.gameRoles = s.gameRoles
}

/**
 * 试玩期间会被脚本事件（background/music/background_effect/present_pic/sound/ambient）
 * 改写的「场景渲染态」。这些不在 gameStore，而在 uiStore + settingsStore：
 * - 背景图、粒子特效存在 **settingsStore.display**（且 settingsStore 是 persist 的），
 *   不还原会写进 localStorage、跨试玩/跨自由对话长期泄漏；
 * - 其余（过渡时长、BGM 轨与速度、插图、音效、环境音轨）在 uiStore。
 *
 * 【还原断言】试玩结束（previewing=false）必须把这一整族还原回试玩前快照，
 * 否则：粒子特效不清空、BGM 不停、背景图/插图/音效串到自由对话或下一次试玩。
 * 新增任何会被脚本事件改写的渲染态字段时，务必同步加进这里存/还。
 */
type SceneSnapshot = {
  // settingsStore.display（持久化，必须还原）
  background: string
  backgroundEffect: string
  // uiStore
  backgroundTransition: number
  backgroundMusic: string
  bgMusicPlaybackRate: number
  presentPic: string
  presentPicScale: number
  // currentSoundEffect 不存：它是「值变化即播放」的一次性触发型字段，
  // 还原成试玩前的路径会误重播；试玩结束直接清成 'None'（见 restoreSceneState）。
  ambientTracks: typeof uiStore.ambientTracks
}

let sceneSnapshot: SceneSnapshot | null = null

const captureSceneState = (): SceneSnapshot => ({
  background: settingsStore.display.currentBackground,
  backgroundEffect: settingsStore.display.backgroundEffect,
  backgroundTransition: uiStore.currentBackgroundTransition,
  backgroundMusic: uiStore.currentBackgroundMusic,
  bgMusicPlaybackRate: uiStore.bgMusicPlaybackRate,
  presentPic: uiStore.currentPresentPic,
  presentPicScale: uiStore.currentPresentPicScale,
  // 深拷贝：ambientTracks 元素是对象，浅拷贝会与试玩期间的操作互相串改
  ambientTracks: uiStore.ambientTracks.map((t) => ({ ...t })),
})

const restoreSceneState = (s: SceneSnapshot) => {
  // settingsStore：直接写字段（与 setCurrentBackground/setBackgroundEffect 等价，但还原走直写更直接）
  settingsStore.display.currentBackground = s.background
  settingsStore.display.backgroundEffect = s.backgroundEffect
  // uiStore
  uiStore.currentBackgroundTransition = s.backgroundTransition
  uiStore.currentBackgroundMusic = s.backgroundMusic
  uiStore.bgMusicPlaybackRate = s.bgMusicPlaybackRate
  uiStore.currentPresentPic = s.presentPic
  uiStore.currentPresentPicScale = s.presentPicScale
  // 音效是触发型字段，直接清成 'None'：GameBackground 的 watch 见 'None' 不会播放，
  // 既不误重播试玩前的音效，也清掉试玩留下的脏路径
  uiStore.currentSoundEffect = 'None'
  uiStore.ambientTracks = s.ambientTracks
}

/**
 * eventQueue 初始是 paused 的 —— 正式游玩里由 LoadingTransition 完成时 resume。
 * 编辑器没有那道转场，所以在预览打开时自己放行；关闭时 clear()，它会同时
 * 清空队列并把 paused 置回 true，免得残留事件泄漏到下一次试玩。
 */
watch(
  () => store.previewing,
  async (on) => {
    if (on) {
      snapshot = captureGameState()
      sceneSnapshot = captureSceneState()
      // 从干净的舞台开始，而不是继承主界面此刻的立绘和台词
      gameStore.presentRoleIds = []
      gameStore.dialogHistory = []
      gameStore.currentLine = ''
      gameStore.currentStatus = 'presenting'

      // 试玩需要 runningScript 非空：choice 处理器要求它存在才会显示选项（issue #4）。
      // 不复用 enterStoryMode：它有 bgMusicMode 等 UI 副作用，这里只要一个最小标记。
      const scriptName = store.detail?.package.scriptName ?? ''
      gameStore.runningScript = {
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

      // 注入主角身份：羁绊剧本的 MAIN 来自绑定角色卡。不设的话玩家气泡空名、
      // 立绘也不会出现（issue #8）。readiness 已在试玩前算好 mainRoleId / userName。
      const r = store.readiness
      if (r?.mainRoleId != null) {
        const id = r.mainRoleId
        gameStore.mainRoleId = id
        gameStore.currentInteractRoleId = id
        gameStore.presentRoleIds = [id]
        if (r.userName) gameStore.userName = r.userName
        // 预载主角的立绘/名字到 gameRoles，否则第一句台词前画面是空的
        try {
          await gameStore.getOrCreateGameRole(id)
        } catch (e) {
          console.warn('[ScriptEditor] 预载主角立绘失败:', e)
        }
      }

      eventQueue.resume()
    } else {
      // clear() 内部会把 paused 置回 true，所以不需要另外 pause
      eventQueue.clear()
      if (snapshot) {
        restoreGameState(snapshot)
        snapshot = null
      }
      // 还原场景渲染态：清掉试玩留下的背景图/粒子特效/BGM/插图/音效/环境音，
      // 否则会跨试玩、跨自由对话泄漏（settingsStore.display 还是 persist 的）。
      if (sceneSnapshot) {
        restoreSceneState(sceneSnapshot)
        sceneSnapshot = null
      }
    }
  },
)
</script>

<style scoped>
.stage {
  position: fixed;
  inset: 0;
  z-index: 9990;
  overflow: hidden;
  background: #000;
}

.bar {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
  z-index: 10000;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  background: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent);
}
.badge {
  border: 1px solid rgba(121, 217, 255, 0.5);
  border-radius: 99px;
  padding: 2px 10px;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--accent-color);
  background: rgba(121, 217, 255, 0.15);
}
.meta {
  font-size: 0.78rem;
  color: #fff;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
}
.tip {
  font-size: 0.7rem;
  color: rgba(255, 255, 255, 0.6);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
}
.stop {
  margin-left: auto;
  border: 1px solid rgba(248, 113, 113, 0.45);
  border-radius: 0.5rem;
  padding: 5px 14px;
  font-size: 0.76rem;
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.16);
  backdrop-filter: blur(8px);
  transition: all 0.2s;
}
.stop:hover {
  color: #fff;
  background: rgba(248, 113, 113, 0.32);
}

.preview-enter-active,
.preview-leave-active {
  transition: opacity 0.25s cubic-bezier(0.18, 0.89, 0.32, 1);
}
.preview-enter-from,
.preview-leave-to {
  opacity: 0;
}
</style>
