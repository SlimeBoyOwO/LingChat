<template>
  <div
    @click="handleDialogueClick"
    class="relative flex items-center justify-center w-full z-30 cursor-pointer transition-all duration-300 ease-out"
    :class="isVisible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2 pointer-events-none'"
  >
    <div
      ref="bubbleRef"
      class="relative w-[85%] rounded-[calc(20px*var(--pet-ui-scale,1))] px-[calc(18px*var(--pet-ui-scale,1))] py-[calc(6px*var(--pet-ui-scale,1))] text-white backdrop-blur-xl backdrop-saturate-200 border bg-neutral-950/50 border-white/10 transition-all duration-300 hover:bg-neutral-950/65 hover:scale-[1.02] hover:-translate-y-0.2 hover:border-white/20 [text-shadow:0_1px_4px_rgba(0,0,0,0.5)]"
      :style="{ maxHeight: `calc(var(--dialog-h) - 20px)` }"
    >
      <div
        class="absolute -bottom-2.5 left-1/2 -translate-x-1/2 w-0 h-0 border-l-10 border-l-transparent border-r-10 border-r-transparent border-t-white/10 drop-shadow-md"
      ></div>
      <div
        class="absolute -bottom-2 left-1/2 -translate-x-1/2 w-0 h-0 border-l-8 border-l-transparent border-r-8 border-r-transparent border-t-8 border-t-white/8"
      ></div>

      <div class="relative overflow-hidden">
        <Transition name="emotion-slide">
          <div
            v-if="characterEmotion"
            :key="characterEmotion"
            class="inline-block max-w-full text-[calc(12px*var(--pet-ui-scale,1))] text-cyan-400 font-semibold italic tracking-wider mb-0.5 drop-shadow-[0_1px_4px_rgba(0,176,255,0.5)] truncate"
          >
            {{ characterEmotion }}
          </div>
        </Transition>
      </div>

      <div
        ref="textareaRef"
        class="dialog-text-lock text-[calc(15px*var(--pet-ui-scale,1))] leading-snug font-medium overflow-y-auto whitespace-pre-line break-all pb-[0.4em] [text-shadow:0_0_3px_rgba(0,0,0,0.9),0_1px_4px_rgba(0,0,0,0.5)]"
        :style="{ maxHeight: `calc(var(--dialog-h) - 52px)` }"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from 'vue'
import { useGameStore } from '../../stores/modules/game'
import { eventQueue } from '../../core/events/event-queue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useSettingsStore } from '../../stores/modules/settings'
import { useFusedStore, type FusedSegment } from '../../stores/modules/ui/fused'
import { useTypeWriter } from '../../composables/ui/useTypeWriter'
import { createCharRevealWriter } from '../../utils/typewriter/charReveal'
import { escapeHtml } from '../../utils/escapeHtml'

const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const fusedStore = useFusedStore()

const emit = defineEmits(['player-continued', 'dialog-proceed'])

const isVisible = computed(() => {
  return gameStore.currentStatus === 'responding' && gameStore.currentLine.trim() !== ''
})

const characterEmotion = computed(() => {
  return uiStore.showCharacterEmotion ? uiStore.showCharacterEmotion : ''
})

// 融合激活(桌宠与主聊共用开关;剧本模式不参与)
const fusedActive = computed(
  () => settingsStore.text.fusedDialogue && !gameStore.runningScript,
)

// ── 台词融合:累积渲染状态(复用主聊思路,动作灰字) ──
// 布局(方案 C):对话区(白字)在上、动作区(灰字)在下,各区内部连续流动。
// 静态/当前段状态存 fused store(staticTextHtml/staticMotionHtml/curText/curMotion):
// 主聊↔桌宠视图切换时新组件从 store 恢复,回复不中断、内容不丢。
/** 单段对话 HTML(白字,段尾 0.8em 间距) */
function fusedTextHtml(text: string): string {
  return `<span style="color:#fff;margin-right:0.8em">${escapeHtml(text)}</span>`
}

/** 单段动作 HTML(灰字,段尾 0.8em 间距) */
function fusedMotionHtml(motion: string): string {
  return `<span style="color:#9ca3af;margin-right:0.8em">${escapeHtml(motion)}</span>`
}

/** 融合布局完整 HTML(方案 C):对话区(静态+当前段)在上,动作区(静态)在下。
 *  writeFn 渲染与高度测量共用,保证锁高与实际显示一致。 */
function fusedRenderHtml(curText: string): string {
  const motionZone = fusedStore.staticMotionHtml ? '<br>' + fusedStore.staticMotionHtml : ''
  return fusedStore.staticTextHtml + fusedTextHtml(curText) + motionZone
}

const handleDialogueClick = () => {
  if (isVisible.value) {
    console.log('点击对话框，继续下一句')
    // 推进统一走 continueDialog(内部负责落定/续打),不再二次
    // eventQueue.continue()——
    // 否则融合中 continueDialog 已续打/等呼吸时,这里的 continue
    // 会把挂起的等点击
    // resolve 掉,直接跳入 input(主聊 sendOrContinue 只调一次,此处对齐)。
    continueDialog(true)
  }
}

const textareaRef = ref<HTMLElement | null>(null)
const bubbleRef = ref<HTMLElement | null>(null)

// 逐字符淡入+上浮渲染器（颜色/阴影继承气泡样式）
const charReveal = createCharRevealWriter({
  charHtml: (char, _index, _rawText, animate) => {
    if (char === '\n') return '<br>'
    if (char === ' ') return ' '
    const anim = animate
      ? ';animation:tw-char-rise .28s cubic-bezier(.22, 1, .36, 1) forwards'
      : ''
    return `<span style="display:inline-block${anim}">${escapeHtml(char)}</span>`
  },
})

const { startTyping, stopTyping, isTyping, finishTyping } = useTypeWriter(
  textareaRef,
  undefined,
  // 融合激活时输出累积 HTML(对话区+动作区);否则逐字符渲染
  (el, text) => {
    if (fusedActive.value) {
      el.innerHTML = fusedRenderHtml(text)
    } else {
      charReveal.writeFn(el, text)
    }
  },
)

/** 开始打一段融合台词 */
function startFusedSegment(text: string, motion: string) {
  fusedStore.curText = text
  fusedStore.curMotion = motion
  startTyping(text, uiStore.typeWriterSpeed)
}

/** 段间呼吸延迟定时器(防重入:延迟期间不重复消费 pending) */
let fusedDelayTimer: ReturnType<typeof setTimeout> | null = null
/** 段间延迟中等待开打的段(卸载时归档放回队首,防视图切换丢段) */
let fusedDelayedSeg: FusedSegment | null = null

/** 融合续打:消费 pending 下一段,由 PetMode tryFusedContinue(媒体完成)调用。
 *  merge=false 的段(独立成句/新台词起点)先清空静态缓冲。
 *  段间 200ms 呼吸:情绪/音频/打字统一延迟,保证切换同步。 */
function continueFused(): boolean {
  if (!fusedActive.value) return false
  if (fusedDelayTimer) return true // 延迟中,防重入
  const seg = fusedStore.shiftPending()
  if (!seg) return false
  // 捕获 pending 代数:角色切换 discardPending 后,延迟回调作废(不打旧角色段)
  const epoch = fusedStore.pendingEpoch
  if (seg.merge) {
    // 合并段:上一段完整展示,归档为静态(对话进对话区,动作进动作区,段尾间距)
    if (fusedStore.curText) {
      fusedStore.staticTextHtml += fusedTextHtml(fusedStore.curText)
      if (fusedStore.curMotion) {
        fusedStore.staticMotionHtml += fusedMotionHtml(fusedStore.curMotion)
      }
      fusedStore.curText = ''
      fusedStore.curMotion = ''
    }
  } else {
    // 独立段:新台词起点,清空静态缓冲
    fusedStore.staticTextHtml = ''
    fusedStore.staticMotionHtml = ''
    fusedStore.curText = ''
    fusedStore.curMotion = ''
  }
  // 200ms 段间呼吸:情绪/立绘 → 音频 → 打字,统一延迟同步切换
  fusedDelayedSeg = seg // 卸载归档依据:延迟中段不随旧组件销毁
  fusedDelayTimer = setTimeout(() => {
    fusedDelayTimer = null
    fusedDelayedSeg = null // 已开打,不再延迟
    if (epoch !== fusedStore.pendingEpoch) return // 延迟中角色切换,丢弃本段
    // 切换该段情绪(合并段在 processor 中未提前设置,桌宠情绪标签读 showCharacterEmotion)
    const role = gameStore.gameRoles[seg.roleId]
    if (role) {
      role.emotion = seg.emotion || '正常'
      role.originalEmotion = seg.originalTag || '正常'
      uiStore.showCharacterEmotion = role.originalEmotion
    }
    if (seg.audioFile) uiStore.currentAvatarAudio = seg.audioFile
    startFusedSegment(seg.text, seg.motionText)
  }, 200)
  return true
}

watch([() => uiStore.showCharacterLine, () => gameStore.currentStatus], ([newLine, newStatus]) => {
  if (newLine && newLine !== '' && newStatus === 'responding') {
    if (fusedActive.value) {
      // 融合:新台词首段 → 清空渲染缓冲(static/cur)
      // 清段间延迟定时器:角色切换后旧段的延迟立即让位,不等 200ms 到期
      if (fusedDelayTimer) {
        clearTimeout(fusedDelayTimer)
        fusedDelayTimer = null
      }
      fusedStore.staticTextHtml = ''
      fusedStore.staticMotionHtml = ''
      fusedStore.curText = ''
      fusedStore.curMotion = ''
      startFusedSegment(newLine, uiStore.showCharacterMotionText)
      return
    }
    // 清空旧行并重置渲染器增量状态，避免新台词被误判为旧文本的延续
    if (textareaRef.value) {
      textareaRef.value.innerHTML = ''
      charReveal.reset()
      // 锁定最终高度：用离屏克隆同步测量完整文本渲染后的高度（受 maxHeight 钳制），
      // 再把真实容器高度设为该值。打字期间盒子不再随逐字换行而跳动；
      // 行与行之间的高度变化由 .dialog-text-lock 的 height 过渡平滑扩展/收缩。
      const el = textareaRef.value
      const clone = el.cloneNode(false) as HTMLDivElement
      clone.style.position = 'fixed'
      clone.style.left = '-9999px'
      clone.style.top = '0'
      clone.style.visibility = 'hidden'
      clone.style.pointerEvents = 'none'
      clone.style.height = 'auto'
      clone.style.overflowY = 'visible'
      clone.style.width = el.clientWidth + 'px'
      el.parentElement?.appendChild(clone)
      charReveal.renderInstant(clone, newLine)
      const finalH = clone.offsetHeight
      clone.remove()
      charReveal.reset()
      el.style.height = finalH + 'px'
    }
    startTyping(newLine, uiStore.typeWriterSpeed)
  } else if (newStatus === 'input') {
    stopTyping()
    if (textareaRef.value) textareaRef.value.style.height = ''
    if (fusedDelayTimer) {
      clearTimeout(fusedDelayTimer)
      fusedDelayTimer = null
    }
    if (textareaRef.value) textareaRef.value.innerHTML = ''
    fusedStore.reset()
  }
})

// 模式切换重挂载：立即从 store 恢复当前台词（不重播打字动画）
onMounted(() => {
  const line = uiStore.showCharacterLine
  if (gameStore.currentStatus === 'responding' && textareaRef.value) {
    if (fusedActive.value) {
      // 融合:从 fused store 还原完整累积状态(静态段 + 当前段),不重播打字。
      // 视图切换已归档段存续在 store,这里整段重建;当前段在打字中则完整展示。
      if (fusedStore.curText || fusedStore.staticTextHtml) {
        textareaRef.value.innerHTML = fusedRenderHtml(fusedStore.curText)
      } else if (line && line !== '') {
        fusedStore.curText = line
        fusedStore.curMotion = uiStore.showCharacterMotionText || ''
        textareaRef.value.innerHTML = fusedRenderHtml(fusedStore.curText)
      }
    } else if (line && line !== '') {
      charReveal.renderInstant(textareaRef.value, line)
    }
  }
})

onUnmounted(() => {
  // 视图切换卸载:清段间延迟,防止回调去调已销毁组件的打字机;
  // 延迟中(已 shift 未开打)的段放回队首,由新视图续打,不丢内容
  if (fusedDelayTimer && fusedDelayedSeg) {
    clearTimeout(fusedDelayTimer)
    fusedDelayTimer = null
    fusedStore.restoreDeferred(fusedDelayedSeg)
    fusedDelayedSeg = null
  }
})

function continueDialog(isPlayerTrigger: boolean): boolean {
  // 融合:用户点击 = 快进当前段(finish + 跳音频),不清空 pending,
  // 剩余段照常播放;pending 空才落定
  if (fusedActive.value) {
    if (isTyping.value) finishTyping()
    uiStore.currentAvatarAudio = 'None'
    fusedStore.audioFinished = true // 置空不触发 audio-ended,手动同步
    fusedStore.markInterrupted()
    if (fusedStore.pendingCount > 0) {
      continueFused()
      return true
    }
    const needWait = eventQueue.continue()
    if (!needWait && isPlayerTrigger) emit('player-continued')
    return needWait
  }
  // 打字中:第一次点击跳过动画、显示完整文本,不推进(与主聊一致)
  if (isTyping.value) {
    finishTyping()
    return false
  }
  const needWait = eventQueue.continue()
  if (!needWait) {
    if (isPlayerTrigger) emit('player-continued')
    emit('dialog-proceed')
  }

  return needWait
}

defineExpose({
  continueDialog,
  continueFused,
  isTyping,
  bubbleRef,
})
</script>

<style scoped>
/* 情绪标签切换：上一个情绪向左滑出，下一个情绪从右侧滑入（推挤效果） */
.emotion-slide-enter-active,
.emotion-slide-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
    opacity 0.3s ease;
}
/* 离开中的旧情绪脱离文档流，覆盖在新情绪上方向左滑出，
 * 容器宽度由新情绪决定 */
.emotion-slide-leave-active {
  position: absolute;
  left: 0;
  top: 0;
}
.emotion-slide-enter-from {
  transform: translateX(100%);
  opacity: 0;
}
.emotion-slide-leave-to {
  transform: translateX(-100%);
  opacity: 0;
}

/* 打字期间高度锁定为最终高度；行切换时高度平滑扩展/收缩 */
.dialog-text-lock {
  transition: height 0.25s cubic-bezier(0.25, 0.46, 0.45, 0.94);
}
</style>

<style>
/* 逐字符淡入+上浮动画。keyframes 必须全局：span 由 JS 动态生成，scoped 选择器无法命中 */
@keyframes tw-char-rise {
  from {
    opacity: 0;
    transform: translateY(0.35em);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
