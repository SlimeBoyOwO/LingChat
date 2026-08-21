<template>
  <div
    v-if="game"
    class="poem-game-overlay"
    :class="{ 'is-corrupted': corrupted, 'is-finishing': finishing }"
    @contextmenu.prevent
    @click.stop
  >
    <div
      class="poem-stage"
      :style="{ backgroundImage: corrupted ? 'none' : `url('${backgroundSrc}')` }"
    >
      <div v-if="flash" class="poem-flash"></div>

      <div class="poem-progress">{{ progressLabel }}</div>

      <div class="poem-words" aria-label="选词写诗">
        <button
          v-for="word in currentWords"
          :key="`${roundIndex}-${word.text}`"
          type="button"
          class="poem-word"
          :class="{ 'glitch-word': word.glitch }"
          :disabled="finishing"
          @click.stop="pickWord(word)"
        >
          {{ displayWord(word) }}
        </button>
      </div>

      <!-- 左下角只保留同一角色；不同倾向的词切换不同差分并触发跳动。
           外层负责待机游走（位移+朝向翻转），内层 img 负责弹跳/hop 动画。 -->
      <div
        v-if="!corrupted"
        class="poem-character"
        :style="{ transform: `translateX(${wanderOffset}px) scaleX(${wanderFlip})` }"
        aria-hidden="true"
      >
        <img
          :src="currentStickerSrc"
          :class="{ hop: hopping !== null, wander: wanderBounce && hopping === null }"
          alt=""
          draggable="false"
          @error="onStickerError"
        />
      </div>

      <!-- 词库损坏后：DDLC 同款——纯白底 + 左下巨大崩坏 sticker（半身出屏）。 -->
      <img
        v-else
        class="poem-broken-sticker"
        :src="brokenStickerSrc"
        alt=""
        draggable="false"
        @error="onBrokenError"
      />

      <div v-if="corrupted" class="poem-corrupt-caption">词库校验失败</div>
      <div v-if="finishing" class="poem-finish-caption">正在保存诗……</div>
    </div>

    <audio ref="audioRef" preload="auto" @timeupdate="maintainLoop"></audio>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import type { ScriptPoemWord } from '@/types/script'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'

type Tone = 'warm' | 'script' | 'void'

const gameStore = useGameStore()
const uiStore = useUIStore()
const audioRef = ref<HTMLAudioElement | null>(null)
const roundIndex = ref(0)
const warmScore = ref(0)
const scriptScore = ref(0)
const voidScore = ref(0)
const hopping = ref<Tone | null>(null)
const currentTone = ref<Tone>('warm')
const corrupted = ref(false)
const finishing = ref(false)
const flash = ref(false)
// DDLC 同款待机游走：每 4~8 秒随机挪一小步（±16px 内随机游走、翻转朝向）并小弹一下。
const wanderOffset = ref(0)
const wanderFlip = ref(1)
const wanderBounce = ref(false)
// 损坏后点词音效状态：baa 彩蛋全局只播一次（对齐原作 played_baa）。
const baaPlayed = ref(false)
// 跳姿/崩坏图缺失时回退到常姿，避免 asset 404 白图。
const hopMissing = ref<Record<Tone, boolean>>({ warm: false, script: false, void: false })
const brokenMissing = ref(false)

let hopTimer = 0
let flashTimer = 0
let fadeTimer = 0
let wanderTimer = 0
let wanderBounceTimer = 0

const game = computed(() => gameStore.poemGame)
const currentWords = computed(() => game.value?.rounds[roundIndex.value] ?? [])
const progressLabel = computed(() => {
  const total = game.value?.rounds.length ?? 20
  const current = Math.min(roundIndex.value + 1, total)
  // 词库损坏后进度显示退化成全 1（原作二周目的计数崩坏彩蛋）。
  if (corrupted.value) return `${'1'.repeat(current)}/${total}`
  return `${current}/${total}`
})
const backgroundSrc = computed(() => toAssetUrl(game.value?.backgroundPath ?? ''))
const warmStickerSrc = computed(() => toAssetUrl(game.value?.warmStickerPath ?? ''))
const scriptStickerSrc = computed(() => toAssetUrl(game.value?.scriptStickerPath ?? ''))
const voidStickerSrc = computed(() => toAssetUrl(game.value?.voidStickerPath ?? ''))
// hop 时换成「-跳」差分（沿用原作 _1/_2 双图切换，而不是纯位移动画）。
const hopStickerSrcs = computed<Record<Tone, string>>(() => ({
  warm: toAssetUrl(hopPathOf(game.value?.warmStickerPath ?? '')),
  script: toAssetUrl(hopPathOf(game.value?.scriptStickerPath ?? '')),
  void: toAssetUrl(hopPathOf(game.value?.voidStickerPath ?? '')),
}))
// 损坏后的巨大崩坏 sticker：由空白差分同目录推导「写诗Q版-崩坏.png」。
const brokenStickerSrc = computed(() =>
  brokenMissing.value
    ? voidStickerSrc.value
    : toAssetUrl(brokenPathOf(game.value?.voidStickerPath ?? '')),
)
const currentStickerSrc = computed(() => {
  const tone = currentTone.value
  if (hopping.value !== null && !hopMissing.value[tone]) return hopStickerSrcs.value[tone]
  if (tone === 'script') return scriptStickerSrc.value
  if (tone === 'void') return voidStickerSrc.value
  return warmStickerSrc.value
})

function toAssetUrl(path: string): string {
  if (!path) return ''
  if (/^(https?:|data:|blob:|asset:)/.test(path)) return path
  return convertFileSrc(path)
}

function hopPathOf(path: string): string {
  return path.replace(/\.png$/i, '-跳.png')
}

function brokenPathOf(path: string): string {
  return path.replace(/写诗Q版-[^/\\]+\.png$/i, '写诗Q版-崩坏.png')
}

// 点词音效从 BGM 路径推导 Sounds 目录（剧本目录结构固定：Assets/Musics、Assets/Sounds）。
function soundPathOf(name: string): string {
  const music = game.value?.musicPath ?? ''
  return music.replace(/[/\\]Musics[/\\][^/\\]+$/, `/Sounds/${name}`)
}

const sfxCache = new Map<string, HTMLAudioElement>()

function playSfx(name: string) {
  const url = toAssetUrl(soundPathOf(name))
  if (!url) return
  let sfx = sfxCache.get(url)
  if (!sfx) {
    sfx = new Audio(url)
    sfx.preload = 'auto'
    sfxCache.set(url, sfx)
  }
  sfx.volume = Math.max(0, Math.min(1, uiStore.backgroundVolume / 100))
  sfx.currentTime = 0
  sfx.play().catch(() => {})
}

// 原作的点词音效规则：正常时 activate_sound；损坏后 randint(0,10) ——
// r==0 且没播过放 baa，r<=5 放 glitch 音，其余静默。
function playPickSfx() {
  if (!corrupted.value) {
    playSfx('select.ogg')
    return
  }
  const r = Math.floor(Math.random() * 11)
  if (r === 0 && !baaPlayed.value) {
    baaPlayed.value = true
    playSfx('baa.ogg')
  } else if (r <= 5) {
    playSfx('select_glitch.ogg')
  }
}

function onStickerError() {
  if (hopping.value !== null) hopMissing.value[currentTone.value] = true
}

function onBrokenError() {
  brokenMissing.value = true
}

function displayWord(word: ScriptPoemWord): string {
  if (!corrupted.value || word.glitch) return word.text
  // 音乐损坏后，偶尔让普通词也少一个字；只改显示，不改计分。
  if ((word.text.codePointAt(0) ?? 0) % 5 !== 0) return word.text
  return word.text.length > 1 ? `${word.text.slice(0, -1)}□` : `${word.text}□`
}

function strongestTone(word: ScriptPoemWord): Tone {
  const scores: Array<[Tone, number]> = [
    ['warm', word.warmPoints],
    ['script', word.scriptPoints],
    ['void', word.voidPoints],
  ]
  scores.sort((a, b) => b[1] - a[1])
  return scores[0]?.[0] ?? 'void'
}

function triggerHop(tone: Tone) {
  clearTimeout(hopTimer)
  hopping.value = null
  requestAnimationFrame(() => {
    hopping.value = tone
    // sticker_hop：easein_quad .18 起跳 + easeout_quad .18 落地，连跳两次，共 0.72s。
    hopTimer = window.setTimeout(() => (hopping.value = null), 720)
  })
}

// 待机游走调度：原作 randomPause(4~8s) → randomMove(随机方向小步) + sticker_move_n(小弹)。
function scheduleWander() {
  clearTimeout(wanderTimer)
  wanderTimer = window.setTimeout(tickWander, 4000 + Math.random() * 4000)
}

function tickWander() {
  if (!game.value || finishing.value) return
  if (hopping.value === null && !corrupted.value) {
    let dir = Math.floor(Math.random() * 3) - 1
    // 原作的折返边界：继续同向会超出 ±5 起步范围就反向。
    if (wanderOffset.value * dir > 5) dir = -dir
    wanderOffset.value += dir * 16
    if (dir > 0) wanderFlip.value = -1
    else if (dir < 0) wanderFlip.value = 1
    wanderBounce.value = true
    clearTimeout(wanderBounceTimer)
    wanderBounceTimer = window.setTimeout(() => (wanderBounce.value = false), 180)
  }
  scheduleWander()
}

async function ensureAudioPlaying() {
  const audio = audioRef.value
  if (!audio || !audio.paused) return
  await audio.play().catch(() => {})
}

async function pickWord(word: ScriptPoemWord) {
  if (finishing.value || !game.value) return
  await ensureAudioPlaying()
  playPickSfx()

  warmScore.value += word.warmPoints
  scriptScore.value += word.scriptPoints
  voidScore.value += word.voidPoints
  // 一次只显示同一角色：词的最高倾向决定本次差分；污染词强制切到空白差分。
  const tone = word.glitch ? 'void' : strongestTone(word)
  currentTone.value = tone

  if (word.glitch && !corrupted.value) {
    // 点到污染词：进入损坏状态——白屏、巨大崩坏 sticker、切故障 BGM；
    // 原作此后再点词不再跳动，只有音效池回应。
    corrupted.value = true
    flash.value = true
    clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (flash.value = false), 180)
    await startTrack(game.value.glitchMusicPath, game.value.glitchLoopStart)
  } else if (!corrupted.value) {
    triggerHop(tone)
  }

  if (roundIndex.value + 1 >= game.value.rounds.length) {
    await finishPoem()
  } else {
    roundIndex.value += 1
  }
}

function winner(): Tone {
  const scores: Array<[Tone, number]> = [
    ['warm', warmScore.value],
    ['script', scriptScore.value],
    ['void', voidScore.value],
  ]
  scores.sort((a, b) => b[1] - a[1])
  return scores[0]?.[0] ?? 'void'
}

async function finishPoem() {
  if (finishing.value) return
  finishing.value = true
  await fadeOut(2000)

  const result = JSON.stringify({
    winner: winner(),
    glitch: corrupted.value,
    warm: warmScore.value,
    script: scriptScore.value,
    void: voidScore.value,
  })

  try {
    await invoke('script_submit_choice', { choice: result })
    gameStore.poemGame = null
  } catch (error) {
    console.error('[PoemGame] 提交结果失败:', error)
    // 保留最后一页，让玩家可以再次点击重试，避免后端永远卡在 oneshot。
    finishing.value = false
  }
}

async function startTrack(path: string, loopStart: number) {
  const audio = audioRef.value
  if (!audio || !path) return
  audio.dataset.loopStart = String(Math.max(0, loopStart || 0))
  audio.pause()
  audio.src = toAssetUrl(path)
  audio.currentTime = 0
  audio.volume = Math.max(0, Math.min(1, uiStore.backgroundVolume / 100))
  await audio.play().catch(() => {})
}

function maintainLoop() {
  const audio = audioRef.value
  if (!audio || !Number.isFinite(audio.duration) || audio.duration <= 0) return
  if (audio.currentTime < audio.duration - 0.08) return
  const loopStart = Number(audio.dataset.loopStart || 0)
  audio.currentTime = Math.min(Math.max(0, loopStart), Math.max(0, audio.duration - 0.1))
  audio.play().catch(() => {})
}

function fadeOut(durationMs: number): Promise<void> {
  return new Promise((resolve) => {
    const audio = audioRef.value
    if (!audio || audio.paused) {
      resolve()
      return
    }
    const startedAt = performance.now()
    const initial = audio.volume
    const tick = () => {
      const ratio = Math.min(1, (performance.now() - startedAt) / durationMs)
      audio.volume = initial * (1 - ratio)
      if (ratio >= 1) {
        audio.pause()
        resolve()
      } else {
        fadeTimer = window.setTimeout(tick, 40)
      }
    }
    tick()
  })
}

function resetGame() {
  clearTimeout(hopTimer)
  clearTimeout(flashTimer)
  clearTimeout(fadeTimer)
  clearTimeout(wanderTimer)
  clearTimeout(wanderBounceTimer)
  roundIndex.value = 0
  warmScore.value = 0
  scriptScore.value = 0
  voidScore.value = 0
  hopping.value = null
  currentTone.value = 'warm'
  corrupted.value = false
  finishing.value = false
  flash.value = false
  wanderOffset.value = 0
  wanderFlip.value = 1
  wanderBounce.value = false
  baaPlayed.value = false
  hopMissing.value = { warm: false, script: false, void: false }
  brokenMissing.value = false
}

watch(
  game,
  async (next) => {
    resetGame()
    const audio = audioRef.value
    if (!next) {
      if (audio) {
        audio.pause()
        audio.removeAttribute('src')
        audio.load()
      }
      return
    }
    await nextTick()
    await startTrack(next.musicPath, next.normalLoopStart)
    scheduleWander()
  },
)

onBeforeUnmount(() => {
  resetGame()
  audioRef.value?.pause()
})
</script>

<style scoped>
.poem-game-overlay {
  position: fixed;
  inset: 0;
  z-index: 950000;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: #07101a;
  pointer-events: auto;
  user-select: none;
}

.poem-stage {
  position: relative;
  width: min(100vw, calc(100vh * 1.7806));
  aspect-ratio: 1672 / 939;
  max-height: 100vh;
  overflow: hidden;
  background-position: center;
  background-repeat: no-repeat;
  background-size: 100% 100%;
  box-shadow: 0 0 80px rgba(0, 0, 0, 0.8);
}

.poem-progress {
  position: absolute;
  /* 贴在右页纸面内，而不是书本上沿；窄窗口缩放时也不会被裁掉。 */
  top: 12.2%;
  right: 42.2%;
  min-width: 6ch;
  text-align: right;
  line-height: 1.15;
  z-index: 2;
  color: #182331;
  font-family: 'Noto Serif SC', 'STKaiti', 'KaiTi', serif;
  font-size: clamp(18px, 2.1vw, 38px);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.05em;
  text-shadow: 0 1px rgba(255, 255, 255, 0.35);
}

.poem-words {
  position: absolute;
  top: 17.5%;
  left: 28.2%;
  width: 30.6%;
  height: 58.8%;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-template-rows: repeat(5, minmax(0, 1fr));
  column-gap: 9%;
  align-items: center;
}

.poem-word {
  appearance: none;
  border: 0;
  padding: 0.25em 0.3em;
  overflow: visible;
  color: #15202b;
  background: transparent;
  font-family: 'Noto Serif SC', 'STKaiti', 'KaiTi', serif;
  font-size: clamp(17px, 1.85vw, 34px);
  line-height: 1;
  text-align: left;
  white-space: nowrap;
  cursor: pointer;
  transition: transform 100ms ease, color 100ms ease, text-shadow 100ms ease;
}

.poem-word:hover,
.poem-word:focus-visible {
  color: #9d3156;
  outline: none;
  transform: translateX(-3px) rotate(-0.5deg);
  text-shadow: 0 0 1px #fff, 0 0 6px rgba(166, 33, 83, 0.5);
}

.poem-word:active {
  transform: scale(0.96);
}

.poem-character {
  position: absolute;
  left: 7.2%;
  bottom: 9.5%;
  width: clamp(68px, 7.5vw, 124px);
  transform-origin: 50% 100%;
  /* 待机游走的位移与朝向翻转走外层容器，平滑过渡。 */
  transition: transform 180ms ease-out;
}

.poem-character img {
  display: block;
  width: 100%;
  height: auto;
  max-height: 21vh;
  object-fit: contain;
  filter: drop-shadow(0 5px 4px rgba(0, 0, 0, 0.32));
  pointer-events: none;
}

/* 待机小弹：sticker_move_n —— easein_quad .08 起、easeout_quad .08 落。 */
.poem-character img.wander {
  animation: ddlc-wander 0.16s;
}

/* 点词跳动：sticker_hop —— 同参数连跳两次（.18+.18）×2，共 0.72s。 */
.poem-character img.hop {
  animation: ddlc-hop 0.72s;
}

/* 损坏态：左下巨大崩坏 sticker（还原 sticker_glitch：xcenter 50 / yalign 1.8 / zoom 3
   —— 中心贴近左边缘、底部约三分之一出屏）。 */
.poem-broken-sticker {
  position: absolute;
  left: -9%;
  top: 60.7%;
  width: 25.8%;
  min-width: 200px;
  pointer-events: none;
  z-index: 2;
}

.is-corrupted .poem-stage {
  background-color: #fff;
}

.glitch-word {
  color: #661821;
  font-weight: 700;
  text-shadow: 2px 0 rgba(0, 130, 170, 0.5), -2px 0 rgba(170, 0, 45, 0.55);
  animation: word-jitter 0.16s steps(2, end) infinite;
}

.poem-corrupt-caption,
.poem-finish-caption {
  position: absolute;
  left: 50.6%;
  bottom: 7.2%;
  color: rgba(83, 24, 35, 0.72);
  font: 600 clamp(10px, 0.9vw, 16px) ui-monospace, monospace;
  letter-spacing: 0.12em;
}

.poem-finish-caption { color: rgba(24, 38, 51, 0.64); }

.poem-flash {
  position: absolute;
  inset: 0;
  z-index: 5;
  background: #fff;
  mix-blend-mode: difference;
  pointer-events: none;
}

.is-corrupted .poem-progress {
  color: #651822;
}

.is-finishing {
  cursor: wait;
}

@keyframes ddlc-wander {
  0%   { transform: translateY(0); animation-timing-function: cubic-bezier(0.11, 0, 0.5, 0); }
  50%  { transform: translateY(-9%); animation-timing-function: cubic-bezier(0.5, 1, 0.89, 1); }
  100% { transform: translateY(0); }
}

@keyframes ddlc-hop {
  0%   { transform: translateY(0); animation-timing-function: cubic-bezier(0.11, 0, 0.5, 0); }
  25%  { transform: translateY(-52%); animation-timing-function: cubic-bezier(0.5, 1, 0.89, 1); }
  50%  { transform: translateY(0); animation-timing-function: cubic-bezier(0.11, 0, 0.5, 0); }
  75%  { transform: translateY(-52%); animation-timing-function: cubic-bezier(0.5, 1, 0.89, 1); }
  100% { transform: translateY(0); }
}

@keyframes word-jitter {
  0% { transform: translate(0, 0); }
  33% { transform: translate(-2px, 1px); }
  66% { transform: translate(2px, -1px); }
}

@media (max-aspect-ratio: 1/1) {
  .poem-word { font-size: clamp(13px, 3vw, 22px); }
}
</style>
