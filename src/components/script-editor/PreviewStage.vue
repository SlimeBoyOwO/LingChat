<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition-opacity duration-[0.25s] ease-[cubic-bezier(0.18,0.89,0.32,1)]"
      leave-active-class="transition-opacity duration-[0.25s] ease-[cubic-bezier(0.18,0.89,0.32,1)]"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div
        v-if="store.previewing"
        class="fixed
          inset-0
          z-[9990]
          overflow-hidden
          bg-black"
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
        <div
          class="absolute
            inset-x-0
            top-0
            z-[10000]
            flex
            items-center
            gap-3
            bg-[linear-gradient(180deg,rgba(0,0,0,0.55),transparent)]
            px-4
            py-2"
        >
          <span
            class="rounded-full
              border
              border-[rgba(121,217,255,0.5)]
              bg-[rgba(121,217,255,0.15)]
              px-2.5
              py-0.5
              text-[0.72rem]
              font-semibold
              text-[var(--accent-color)]"
            >{{ t('scriptEditor.previewStage.playing') }}</span
          >
          <span class="text-[0.78rem]
            text-white
            [text-shadow:0_1px_3px_rgba(0,0,0,0.6)]">{{
            label
          }}</span>
          <span class="text-[0.7rem]
            text-white/[0.6]
            [text-shadow:0_1px_3px_rgba(0,0,0,0.6)]">{{
            t('scriptEditor.previewStage.debugNotice')
          }}</span>
          <button
            class="ml-auto
              rounded-lg
              border
              border-[rgba(248,113,113,0.45)]
              bg-[rgba(248,113,113,0.16)]
              px-[14px]
              py-[5px]
              text-[0.76rem]
              text-[#fca5a5]
              backdrop-blur-[8px]
              transition-all
              hover:text-white
              hover:bg-[rgba(248,113,113,0.32)]"
            title="Esc"
            @click="store.stopPreview()"
          >
            {{ t('scriptEditor.previewStage.stop') }}
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { GameBackground, GameDialog, GameRolesStage } from '@/components/game/standard'
import GameExtraUI from '@/components/game/standard/GameExtraUI.vue'
import { eventQueue } from '@/core/events/event-queue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'

const { t } = useI18n()
const store = useScriptEditorStore()
const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

const props = defineProps<{ fromChapter?: string }>()

const label = computed(() => {
  const parts = [store.detail?.package.scriptName ?? '']
  if (props.fromChapter)
    parts.push(t('scriptEditor.previewStage.fromChapter', { chapter: props.fromChapter }))
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
  /** 玩家副标题（试玩时可能被剧本/角色卡覆盖），退出时还原，否则不同角色标题混搭 */
  userSubtitle: string
  /** 当前场景（光照/滤镜作用域），脚本场景会影响后续自由对话立绘可见性 */
  currentScene: typeof gameStore.currentScene
  currentLine: string
  currentStatus: typeof gameStore.currentStatus
  dialogHistory: typeof gameStore.dialogHistory
  command: string | null
  /** 试玩会往角色缓存里塞剧本角色，退出时要还回原样，否则回自由对话立绘会串/消失 */
  gameRoles: typeof gameStore.gameRoles
  /** 标记游戏是否已初始化。MainChat 据此决定是否重跑 initializeGame，
   *  退出编辑器时会在 leave() 里设为 false，这里存一下保持语义一致 */
  initialized: boolean
}

let snapshot: GameSnapshot | null = null

const captureGameState = (): GameSnapshot => ({
  runningScript: gameStore.runningScript,
  presentRoleIds: [...gameStore.presentRoleIds],
  currentInteractRoleId: gameStore.currentInteractRoleId,
  mainRoleId: gameStore.mainRoleId,
  userName: gameStore.userName,
  userSubtitle: gameStore.userSubtitle,
  currentScene: gameStore.currentScene,
  currentLine: gameStore.currentLine,
  currentStatus: gameStore.currentStatus,
  dialogHistory: [...gameStore.dialogHistory],
  command: gameStore.command,
  // 深拷贝每个角色对象（含嵌套的 clothes/bodyPart），JSON 序列化确保完全切断
  // 共享引用。浅拷贝 { ...role } 仍会共用嵌套对象，试玩期间修改 clothes 等会污染快照。
  gameRoles: JSON.parse(JSON.stringify(gameStore.gameRoles)),
  initialized: gameStore.initialized,
})

const restoreGameState = (s: GameSnapshot) => {
  gameStore.runningScript = s.runningScript
  gameStore.presentRoleIds = s.presentRoleIds
  gameStore.currentInteractRoleId = s.currentInteractRoleId
  gameStore.mainRoleId = s.mainRoleId
  gameStore.userName = s.userName
  gameStore.userSubtitle = s.userSubtitle
  gameStore.currentScene = s.currentScene
  gameStore.currentLine = s.currentLine
  gameStore.currentStatus = s.currentStatus
  gameStore.dialogHistory = s.dialogHistory
  gameStore.command = s.command
  gameStore.gameRoles = s.gameRoles
  gameStore.initialized = s.initialized
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
  // 角色标题/副标题：试玩期间脚本对话会把它们改成剧本 NPC 的名字，
  // 不还原的话回自由对话仍显示错误的角色身份
  showCharacterTitle: string
  showCharacterSubtitle: string
  // 台词/情绪/动作文本：dialogue-processor 试玩期间逐句改写，纯展示字段，还回原值
  showCharacterLine: string
  showCharacterEmotion: string
  showCharacterMotionText: string
  // currentSoundEffect 不存：它是「值变化即播放」的一次性触发型字段，
  // 还原成试玩前的路径会误重播；试玩结束直接清成 'None'（见 restoreSceneState）。
  // currentAvatarAudio 同理（角色语音也是值变化即播），一并只清不存。
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
  showCharacterTitle: uiStore.showCharacterTitle,
  showCharacterSubtitle: uiStore.showCharacterSubtitle,
  showCharacterLine: uiStore.showCharacterLine,
  showCharacterEmotion: uiStore.showCharacterEmotion,
  showCharacterMotionText: uiStore.showCharacterMotionText,
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
  uiStore.showCharacterTitle = s.showCharacterTitle
  uiStore.showCharacterSubtitle = s.showCharacterSubtitle
  uiStore.showCharacterLine = s.showCharacterLine
  uiStore.showCharacterEmotion = s.showCharacterEmotion
  uiStore.showCharacterMotionText = s.showCharacterMotionText
  // 音效是触发型字段，直接清成 'None'：GameBackground 的 watch 见 'None' 不会播放，
  // 既不误重播试玩前的音效，也清掉试玩留下的脏路径
  uiStore.currentSoundEffect = 'None'
  // 角色语音同理：还原成试玩前的路径会误重播自由对话最后一句，直接清 'None'
  uiStore.currentAvatarAudio = 'None'
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
      // 先清掉上一轮试玩可能残留在事件队列里的事件（如 show_character），
      // 否则新试玩开始后它们还会被处理，把旧角色注入到当前舞台。
      eventQueue.clear()
      snapshot = captureGameState()
      sceneSnapshot = captureSceneState()
      // 从干净的舞台开始，而不是继承主界面此刻的立绘和台词。
      // 清空整个角色缓存以确保多轮试玩之间不会残留前一回合剧本添加的角色
      // 对象和立绘状态（否则切到不同剧本试玩时可能出现多角色站位残留）。
      gameStore.presentRoleIds = []
      gameStore.gameRoles = {}
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
        // 试玩中玩家副标题用玩家名作兜底，避免 player 事件的 displaySubtitle 为空时字幕丢失；
        // 玩家名也为空时用「玩家」保底，保证字幕栏始终有内容
        gameStore.userSubtitle =
          r.userName || gameStore.userSubtitle || t('scriptEditor.previewStage.player')
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
