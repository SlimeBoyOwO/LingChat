<template>
  <!--
    Teleport 到 #app 而不是 body：全局 Ctrl+滚轮缩放是 CSS zoom 打在 #app 上
    （useZoom），挂到 body 的浮层会脱离缩放作用域，永远按 100% 视口渲染，缩放后
    与编辑器主体比例失调。试玩层是编辑器最常用的浮层，必须与主体等比缩放。
  -->
  <Teleport to="#app">
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

const { t } = useI18n()
const store = useScriptEditorStore()
const gameStore = useGameStore()

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
 * 试玩进出场的状态备份/还原已迁入 script-editor store（capturePreviewGameState /
 * capturePreviewSceneState / restorePreviewState）：
 *
 * - 后端已经把 `GameStatus` 整个备份还原了（见 `PreviewSession`），但前端这份是
 *   独立的一套状态：立绘在场名单、对话历史、剧情模式标记都只存在于浏览器里，
 *   引擎 emit 的事件经 eventQueue 直接改它。不管的话，退出编辑器回自由对话，
 *   看到的还是试玩留下的立绘和台词 —— 包括「AI 已关闭」那几条占位。
 * - 场景渲染态（背景/BGM/特效等）在 uiStore + settingsStore（persist），不还原
 *   会跨试玩长期泄漏。
 * - 搭新场必须早于 `editor_start_preview` invoke（引擎在其中就 spawn 并 emit，
 *   晚于 invoke 的清场会丢 free_dialogue 等开头事件），所以由 startPreview 在
 *   invoke 前调用 store.preparePreviewState()；本组件只在退出时还原。
 */

/**
 * eventQueue 初始是 paused 的 —— 正式游玩里由 LoadingTransition 完成时 resume。
 * 编辑器没有那道转场，所以在预览打开时自己放行；关闭时 clear()，它会同时
 * 清空队列并把 paused 置回 true，免得残留事件泄漏到下一次试玩。
 */
watch(
  () => store.previewing,
  async (on) => {
    if (on) {
      // 搭场（快照/清队/清舞台/建 runningScript）已在 startPreview 中完成

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
      store.restorePreviewState()
    }
  },
)
</script>
