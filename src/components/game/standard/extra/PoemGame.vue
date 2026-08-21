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
      :style="{ backgroundImage: `url('${backgroundSrc}')` }"
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

      <!-- 左下角只保留同一角色；不同倾向的词切换不同差分并触发跳动。 -->
      <div
        class="poem-character"
        :class="{ hop: hopping !== null, 'is-void': currentTone === 'void' }"
        aria-hidden="true"
      >
        <img :src="currentStickerSrc" alt="" draggable="false" />
      </div>

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

let hopTimer = 0
let flashTimer = 0
let fadeTimer = 0

const game = computed(() => gameStore.poemGame)
const currentWords = computed(() => game.value?.rounds[roundIndex.value] ?? [])
const progressLabel = computed(() => {
  const total = game.value?.rounds.length ?? 20
  return `${Math.min(roundIndex.value + 1, total)}/${total}`
})
const backgroundSrc = computed(() => toAssetUrl(game.value?.backgroundPath ?? ''))
const warmStickerSrc = computed(() => toAssetUrl(game.value?.warmStickerPath ?? ''))
const scriptStickerSrc = computed(() => toAssetUrl(game.value?.scriptStickerPath ?? ''))
const voidStickerSrc = computed(() => toAssetUrl(game.value?.voidStickerPath ?? ''))
const currentStickerSrc = computed(() => {
  if (currentTone.value === 'script') return scriptStickerSrc.value
  if (currentTone.value === 'void') return voidStickerSrc.value
  return warmStickerSrc.value
})

function toAssetUrl(path: string): string {
  if (!path) return ''
  if (/^(https?:|data:|blob:|asset:)/.test(path)) return path
  return convertFileSrc(path)
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
    hopTimer = window.setTimeout(() => (hopping.value = null), 520)
  })
}

async function ensureAudioPlaying() {
  const audio = audioRef.value
  if (!audio || !audio.paused) return
  await audio.play().catch(() => {})
}

async function pickWord(word: ScriptPoemWord) {
  if (finishing.value || !game.value) return
  await ensureAudioPlaying()

  warmScore.value += word.warmPoints
  scriptScore.value += word.scriptPoints
  voidScore.value += word.voidPoints
  // 一次只显示同一角色：词的最高倾向决定本次差分；污染词强制切到空白差分。
  const tone = word.glitch ? 'void' : strongestTone(word)
  currentTone.value = tone
  triggerHop(tone)

  if (word.glitch && !corrupted.value) {
    corrupted.value = true
    flash.value = true
    clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (flash.value = false), 180)
    await startTrack(game.value.glitchMusicPath, game.value.glitchLoopStart)
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
  roundIndex.value = 0
  warmScore.value = 0
  scriptScore.value = 0
  voidScore.value = 0
  hopping.value = null
  currentTone.value = 'warm'
  corrupted.value = false
  finishing.value = false
  flash.value = false
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

.poem-character.hop {
  animation: marker-hop 0.48s cubic-bezier(0.3, 0.85, 0.35, 1);
}

.is-corrupted .poem-character.is-void.hop {
  animation: marker-glitch-hop 0.38s steps(2, end);
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

.is-corrupted .poem-stage {
  animation: stage-breathe 3.4s ease-in-out infinite;
}

.is-corrupted .poem-progress {
  color: #651822;
}

.is-finishing {
  cursor: wait;
}

@keyframes marker-hop {
  0%, 100% { transform: translateY(0) rotate(0); }
  35% { transform: translateY(-48%) rotate(-4deg); }
  62% { transform: translateY(-8%) rotate(3deg); }
  80% { transform: translateY(-28%) rotate(-2deg); }
}

@keyframes word-jitter {
  0% { transform: translate(0, 0); }
  33% { transform: translate(-2px, 1px); }
  66% { transform: translate(2px, -1px); }
}

@keyframes marker-glitch-hop {
  0%, 100% { transform: translate(0, 0) scale(1); filter: invert(0); }
  25% { transform: translate(-8%, -42%) scale(1.16, 0.84); filter: invert(1); }
  50% { transform: translate(7%, -12%) scale(0.88, 1.18); filter: invert(0); }
  75% { transform: translate(-3%, -30%) scale(1.1, 0.9); filter: invert(1); }
}

@keyframes stage-breathe {
  0%, 100% { filter: none; transform: translate(0, 0); }
  48% { filter: contrast(1.04) saturate(0.86); transform: translate(0, 0); }
  50% { filter: contrast(1.18) saturate(0.65); transform: translate(-1px, 1px); }
  52% { filter: contrast(1.04) saturate(0.86); transform: translate(0, 0); }
}

@media (max-aspect-ratio: 1/1) {
  .poem-word { font-size: clamp(13px, 3vw, 22px); }
}
</style>
