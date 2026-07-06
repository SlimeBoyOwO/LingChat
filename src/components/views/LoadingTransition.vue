<template>
  <!-- 全屏加载过渡层 -->
  <div class="loading-transition-root">
    <!-- 1. 全黑入场遮罩 -->
    <div
      v-if="entranceActive"
      :class="[
        'fixed inset-0 bg-black z-9999 transition-opacity duration-1000 ease-out pointer-events-none',
        entranceFadeOut ? 'opacity-0' : 'opacity-100',
      ]"
    ></div>

    <!-- 2. SVG 遮罩定义区 -->
    <svg width="0" height="0" class="absolute">
      <defs>
        <mask id="cat-mask" maskContentUnits="objectBoundingBox" x="0" y="0" width="1" height="1">
          <rect width="1" height="1" fill="white" />
          <g id="mask-anim-group">
            <path
              d="M 0.25,1.0 C 0.25,0.85 0.28,0.80 0.32,0.78 C 0.30,0.68 0.28,0.58 0.34,0.58 C 0.38,0.58 0.42,0.70 0.45,0.75 C 0.47,0.73 0.53,0.73 0.55,0.75 C 0.58,0.70 0.62,0.58 0.66,0.58 C 0.72,0.58 0.70,0.68 0.68,0.78 C 0.72,0.80 0.75,0.85 0.75,1.0 Z"
              fill="black"
            />
          </g>
        </mask>
      </defs>
    </svg>

    <!-- 3. 加载面板 (带 SVG 遮罩) -->
    <div
      v-if="!loadingDestroyed"
      class="masked-loading fixed inset-0 z-9998 bg-[#070f15] flex flex-col items-center justify-between py-12 px-6 overflow-hidden"
    >
      <!-- 顶部状态条 -->
      <div
        class="w-full max-w-6xl flex justify-between items-center z-10 text-sm tracking-wider text-teal-400 opacity-80 px-4"
      >
        <div class="flex items-center space-x-2">
          <span class="inline-block w-2 h-2 rounded-full bg-teal-400 animate-pulse"></span>
          <span>LingChat 终端机启动ing...</span>
        </div>
        <div class="flex items-center space-x-4">
          <span>⩌⩊⩌ 小可爱马上准备就绪~</span>
        </div>
      </div>

      <!-- 中心：猫爪 + 进度圈 -->
      <div class="relative flex flex-col items-center justify-center my-auto z-10">
        <div class="absolute w-72 h-72 rounded-full bg-cyan-500/5 blur-3xl animate-pulse"></div>

        <div class="relative w-64 h-64 flex items-center justify-center">
          <svg
            class="absolute w-full h-full transform -rotate-90 overflow-visible"
            viewBox="0 0 100 100"
          >
            <circle
              cx="50"
              cy="50"
              r="44"
              stroke="rgba(45, 212, 191, 0.08)"
              stroke-width="2"
              fill="none"
            />
            <circle
              cx="50"
              cy="50"
              r="44"
              stroke="url(#cyanGrad)"
              stroke-width="3"
              fill="none"
              :stroke-dasharray="276"
              :stroke-dashoffset="276 - (276 * progress) / 100"
              stroke-linecap="round"
              class="transition-all duration-100"
            />
            <circle
              cx="50"
              cy="50"
              r="48"
              stroke="rgba(45, 212, 191, 0.15)"
              stroke-width="1"
              fill="none"
            />
            <circle
              cx="50"
              cy="50"
              r="51"
              stroke="rgba(45, 212, 191, 0.10)"
              stroke-width="0.8"
              fill="none"
            />
            <defs>
              <linearGradient id="cyanGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#22d3ee" />
                <stop offset="100%" stop-color="#2dd4bf" />
              </linearGradient>
            </defs>
          </svg>

          <!-- 光环粒子：内环 -->
          <div class="absolute w-[250px] h-[250px] animate-spin" style="animation-duration: 5s">
            <span
              class="absolute top-0 left-1/2 -translate-x-1/2 -translate-y-1/2 w-2.5 h-2.5 rounded-full bg-cyan-400 glow-cyan"
            ></span>
          </div>
          <!-- 光环粒子：外环 -->
          <div class="absolute w-[265px] h-[265px] animate-spin" style="animation-duration: 3s">
            <span
              class="absolute top-0 left-1/2 -translate-x-1/2 -translate-y-1/2 w-2 h-2 rounded-full bg-teal-400 opacity-80"
            ></span>
          </div>

          <!-- 赛博猫爪 -->
          <div class="absolute w-36 h-36 flex items-center justify-center animate-float">
            <svg class="w-full h-full text-cyan-400 glow-cyan" viewBox="0 0 100 100">
              <ellipse
                cx="23"
                cy="46"
                rx="7"
                ry="11"
                class="fill-current transition-all duration-300"
                :class="progress > 20 ? 'opacity-100 scale-100' : 'opacity-25 scale-90'"
              />
              <ellipse
                cx="40"
                cy="28"
                rx="8.5"
                ry="13"
                class="fill-current transition-all duration-300"
                :class="progress > 40 ? 'opacity-100 scale-100' : 'opacity-25 scale-90'"
              />
              <ellipse
                cx="60"
                cy="28"
                rx="8.5"
                ry="13"
                class="fill-current transition-all duration-300"
                :class="progress > 60 ? 'opacity-100 scale-100' : 'opacity-25 scale-90'"
              />
              <ellipse
                cx="77"
                cy="46"
                rx="7"
                ry="11"
                class="fill-current transition-all duration-300"
                :class="progress > 80 ? 'opacity-100 scale-100' : 'opacity-25 scale-90'"
              />
              <path
                d="M 28,70 C 24,56 36,50 50,50 C 64,50 76,56 72,70 C 68,81 58,79 50,79 C 42,79 32,81 28,70 Z"
                class="fill-current transition-all duration-300"
                :class="progress > 10 ? 'opacity-100 scale-100' : 'opacity-25 scale-95'"
              />
            </svg>
          </div>
        </div>

        <!-- 状态文本 -->
        <div class="mt-8 flex flex-col items-center space-y-2">
          <div class="h-8 flex items-center">
            <span
              v-if="!isEstablished"
              class="text-2xl font-mono tracking-[0.2em] text-cyan-300 text-glow-teal animate-pulse"
            >
              CONNECTING<span class="inline-block w-4 text-left">{{ dots }}</span>
            </span>
            <span
              v-else
              class="text-2xl font-bold font-mono tracking-[0.1em] text-emerald-400 text-glow-teal flex items-center space-x-2"
            >
              <svg
                class="w-6 h-6 animate-bounce"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="3"
                  d="M5 13l4 4L19 7"
                />
              </svg>
              <span>CONNECTION ESTABLISHED</span>
            </span>
          </div>

          <div class="w-[30vh] flex justify-between text-sm font-mono text-cyan-500/50">
            <span>建立连接中...</span>
            <span>TCP握手中...</span>
            <span>心灵链接中...</span>
          </div>

          <!-- 进度条 -->
          <div class="w-[60vh] flex flex-col items-center space-y-2 mt-4">
            <div
              class="w-[60vh] h-6 bg-slate-950/80 rounded-full border border-teal-500/20 p-0.5 relative overflow-hidden flex items-center"
            >
              <div
                class="h-full bg-gradient-to-r from-teal-500/80 to-cyan-400 rounded-full glow-cyan transition-all duration-100"
                :style="{ width: progress + '%' }"
              >
                <div class="w-full h-0.5 bg-white/30 rounded-full"></div>
              </div>
            </div>
          </div>

          <div class="w-[40vh] flex justify-center gap-12 text-sm font-mono text-cyan-300/80">
            <span class="text-cyan-500/80">长时间工作的话，不要忘记喝水~</span>
            <span>{{ Math.floor(progress) }}%</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'

const emit = defineEmits<{
  complete: []
}>()

// ============================================================
//  Web Audio 音效合成器
// ============================================================
const NekoSynth = {
  ctx: null as AudioContext | null,
  isMuted: false,

  init() {
    if (!this.ctx) {
      this.ctx = new (window.AudioContext || (window as any).webkitAudioContext)()
    }
  },

  playTick() {
    if (this.isMuted || !this.ctx) return
    this.init()
    const osc = this.ctx!.createOscillator()
    const gain = this.ctx!.createGain()
    osc.type = 'sine'
    osc.frequency.setValueAtTime(1400, this.ctx!.currentTime)
    gain.gain.setValueAtTime(0.015, this.ctx!.currentTime)
    gain.gain.exponentialRampToValueAtTime(0.0001, this.ctx!.currentTime + 0.04)
    osc.connect(gain)
    gain.connect(this.ctx!.destination)
    osc.start()
    osc.stop(this.ctx!.currentTime + 0.05)
  },

  playChime() {
    if (this.isMuted || !this.ctx) return
    this.init()
    const now = this.ctx!.currentTime
    const chords = [523.25, 659.25, 783.99, 1046.5]
    chords.forEach((freq, i) => {
      const osc = this.ctx!.createOscillator()
      const gain = this.ctx!.createGain()
      osc.type = 'sine'
      osc.frequency.setValueAtTime(freq, now + i * 0.08)
      gain.gain.setValueAtTime(0, now)
      gain.gain.linearRampToValueAtTime(0.06, now + i * 0.08 + 0.02)
      gain.gain.exponentialRampToValueAtTime(0.0001, now + i * 0.08 + 0.35)
      osc.connect(gain)
      gain.connect(this.ctx!.destination)
      osc.start(now + i * 0.08)
      osc.stop(now + i * 0.08 + 0.4)
    })
  },

  playPop() {
    if (this.isMuted || !this.ctx) return
    this.init()
    const osc = this.ctx!.createOscillator()
    const gain = this.ctx!.createGain()
    osc.type = 'sine'
    osc.frequency.setValueAtTime(260, this.ctx!.currentTime)
    osc.frequency.exponentialRampToValueAtTime(780, this.ctx!.currentTime + 0.22)
    gain.gain.setValueAtTime(0.04, this.ctx!.currentTime)
    gain.gain.exponentialRampToValueAtTime(0.0001, this.ctx!.currentTime + 0.22)
    osc.connect(gain)
    gain.connect(this.ctx!.destination)
    osc.start()
    osc.stop(this.ctx!.currentTime + 0.23)
  },

  playUnveil() {
    if (this.isMuted || !this.ctx) return
    this.init()
    const osc = this.ctx!.createOscillator()
    const gain = this.ctx!.createGain()
    osc.type = 'triangle'
    osc.frequency.setValueAtTime(600, this.ctx!.currentTime)
    osc.frequency.exponentialRampToValueAtTime(60, this.ctx!.currentTime + 1.2)
    gain.gain.setValueAtTime(0.08, this.ctx!.currentTime)
    gain.gain.exponentialRampToValueAtTime(0.0001, this.ctx!.currentTime + 1.2)
    osc.connect(gain)
    gain.connect(this.ctx!.destination)
    osc.start()
    osc.stop(this.ctx!.currentTime + 1.21)
  },
}

// ============================================================
//  动画状态
// ============================================================
const entranceActive = ref(true)
const entranceFadeOut = ref(false)
const progress = ref(0)
const isEstablished = ref(false)
const dots = ref('.')
const isPeeking = ref(false)
const isUnveiling = ref(false)
const loadingDestroyed = ref(false)

// ============================================================
//  SVG 遮罩动画核心
// ============================================================
const CX = 0.5
const CY = 0.79

let maskRafId: number | null = null

function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t
}

function buildMaskTransform(ty: number, sx: number, sy: number): string {
  return [
    'translate(',
    CX.toFixed(4),
    ', ',
    (ty + CY).toFixed(4),
    ') scale(',
    sx,
    ', ',
    sy,
    ') translate(',
    (-CX).toFixed(4),
    ', ',
    (-CY).toFixed(4),
    ')',
  ].join('')
}

function setMaskTransform(ty: number, sx: number, sy: number) {
  const el = document.getElementById('mask-anim-group')
  if (el) el.setAttribute('transform', buildMaskTransform(ty, sx, sy))
}

function easeOutBack(t: number): number {
  const c1 = 1.70158
  const c3 = c1 + 1
  return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2)
}

function easeInOutQuad(t: number): number {
  return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2
}

// 阶段 1：猫耳从底部贝塞尔式弹出
function animatePeek(onComplete?: () => void) {
  const el = document.getElementById('mask-anim-group')
  if (!el) {
    onComplete?.()
    return
  }

  const fromY = 0.52,
    toY = 0.28
  const duration = 550
  const startTime = performance.now()

  function step(now: number) {
    const t = Math.min((now - startTime) / duration, 1)
    const ty = fromY + (toY - fromY) * easeOutBack(t)
    el!.setAttribute('transform', buildMaskTransform(ty, 1, 1))
    if (t < 1) {
      maskRafId = requestAnimationFrame(step)
    } else {
      maskRafId = null
      onComplete?.()
    }
  }
  maskRafId = requestAnimationFrame(step)
}

// 阶段 2：停顿（由 setTimeout 处理，此处无额外逻辑）

// 阶段 3：蓄力下压
function animateAnticipation(onComplete?: () => void) {
  const el = document.getElementById('mask-anim-group')
  if (!el) {
    onComplete?.()
    return
  }

  const fromY = 0.28,
    toY = 0.37
  const fromSX = 1,
    toSX = 1.08
  const fromSY = 1,
    toSY = 0.88
  const duration = 350
  const startTime = performance.now()

  function step(now: number) {
    const t = Math.min((now - startTime) / duration, 1)
    const e = easeInOutQuad(t)
    const ty = lerp(fromY, toY, e)
    const sx = lerp(fromSX, toSX, e)
    const sy = lerp(fromSY, toSY, e)
    el!.setAttribute('transform', buildMaskTransform(ty, sx, sy))
    if (t < 1) {
      maskRafId = requestAnimationFrame(step)
    } else {
      maskRafId = null
      onComplete?.()
    }
  }
  maskRafId = requestAnimationFrame(step)
}

// 阶段 4：最终揭幕
function animateUnveil(onComplete?: () => void) {
  const el = document.getElementById('mask-anim-group')
  if (!el) {
    onComplete?.()
    return
  }

  const keyframes = [
    { t: 0.0, ty: 0.37, sx: 1.08, sy: 0.88 },
    { t: 0.1, ty: 0.0, sx: 1.15, sy: 1.15 },
    { t: 1.0, ty: -4.0, sx: 80.0, sy: 80.0 },
  ]

  const duration = 1100
  const startTime = performance.now()

  function step(now: number) {
    const raw = Math.min((now - startTime) / duration, 1)

    let i = 0
    while (i < keyframes.length - 1 && keyframes[i + 1].t < raw) i++
    const k0 = keyframes[i],
      k1 = keyframes[i + 1]
    const lt = (raw - k0.t) / (k1.t - k0.t)

    const ty = lerp(k0.ty, k1.ty, lt)
    const sx = lerp(k0.sx, k1.sx, lt)
    const sy = lerp(k0.sy, k1.sy, lt)

    el!.setAttribute('transform', buildMaskTransform(ty, sx, sy))

    if (raw < 1) {
      maskRafId = requestAnimationFrame(step)
    } else {
      maskRafId = null
      onComplete?.()
    }
  }
  maskRafId = requestAnimationFrame(step)
}

function cancelMaskAnimation() {
  if (maskRafId) {
    cancelAnimationFrame(maskRafId)
    maskRafId = null
  }
}

// ============================================================
//  打字点点动画
// ============================================================
let dotTimer: ReturnType<typeof setInterval> | null = null

function startDotAnimation() {
  dotTimer = setInterval(() => {
    dots.value = dots.value.length >= 3 ? '.' : dots.value + '.'
  }, 450)
}

// ============================================================
//  进度条 + 转场序列
// ============================================================
let progressInterval: ReturnType<typeof setInterval> | null = null
let fallbackTimer: ReturnType<typeof setTimeout> | null = null

const MAX_PROGRESS_DURATION = 2000 // 2 秒强制完成

function handleTransitionSequence() {
  if (dotTimer) clearInterval(dotTimer)
  isEstablished.value = true
  NekoSynth.playChime()

  // 等 1.2s 让用户看到 "Connection Established" 后开始转场
  setTimeout(() => {
    // 阶段 1：猫耳弹出
    isPeeking.value = true
    NekoSynth.playPop()
    animatePeek(() => {
      // 阶段 2：短暂停顿 500ms
      setTimeout(() => {
        // 阶段 3：蓄力下压
        isPeeking.value = false
        isUnveiling.value = true
        animateAnticipation(() => {
          // 阶段 4：爆发展开揭幕
          NekoSynth.playUnveil()
          animateUnveil(() => {
            // 动画完成 → 销毁加载 DOM → 通知父组件
            setTimeout(() => {
              loadingDestroyed.value = true
              emit('complete')
            }, 100)
          })
        })
      }, 500)
    })
  }, 1200)
}

function startProgress() {
  // 清除旧的定时器
  if (progressInterval) clearInterval(progressInterval)
  if (fallbackTimer) clearTimeout(fallbackTimer)

  // 2 秒强制完成定时器
  fallbackTimer = setTimeout(() => {
    if (progress.value < 100) {
      // 快速补完进度
      if (progressInterval) clearInterval(progressInterval)
      progressInterval = setInterval(() => {
        if (progress.value < 100) {
          progress.value = Math.min(100, progress.value + 5)
        } else {
          if (progressInterval) clearInterval(progressInterval)
          handleTransitionSequence()
        }
      }, 30)
    }
  }, MAX_PROGRESS_DURATION)

  // 正常进度模拟
  progressInterval = setInterval(() => {
    if (progress.value < 100) {
      const step = Math.random() * 2.8 + 0.6
      progress.value = Math.min(100, progress.value + step)
      if (Math.random() > 0.8) NekoSynth.playTick()
    } else {
      if (progressInterval) clearInterval(progressInterval)
      if (fallbackTimer) clearTimeout(fallbackTimer)
      handleTransitionSequence()
    }
  }, 80)
}

// ============================================================
//  生命周期
// ============================================================
onMounted(() => {
  // 初始化遮罩位置：猫头完全隐藏在屏幕下方
  setMaskTransform(0.52, 1, 1)

  // 入场动画：短暂黑屏 → 淡出
  setTimeout(() => {
    entranceFadeOut.value = true
    setTimeout(() => {
      entranceActive.value = false
    }, 1000)
  }, 300)

  startDotAnimation()
  startProgress()
})

onUnmounted(() => {
  if (progressInterval) clearInterval(progressInterval)
  if (fallbackTimer) clearTimeout(fallbackTimer)
  if (dotTimer) clearInterval(dotTimer)
  cancelMaskAnimation()
})
</script>

<style scoped>
/* ===== 赛博网格背景 ===== */
.bg-grid {
  background-size: 40px 40px;
  background-image:
    linear-gradient(to right, rgba(45, 212, 191, 0.05) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(45, 212, 191, 0.05) 1px, transparent 1px);
}

/* ===== 扫描线 ===== */
.scanline::before {
  content: ' ';
  display: block;
  position: absolute;
  top: 0;
  left: 0;
  bottom: 0;
  right: 0;
  background:
    linear-gradient(rgba(18, 16, 16, 0) 50%, rgba(0, 0, 0, 0.25) 50%),
    linear-gradient(90deg, rgba(255, 0, 0, 0.06), rgba(0, 255, 0, 0.02), rgba(0, 0, 255, 0.06));
  z-index: 100;
  background-size:
    100% 2px,
    3px 100%;
  pointer-events: none;
}

/* ===== 霓虹发光 ===== */
.glow-cyan {
  filter: drop-shadow(0 0 10px rgba(34, 211, 238, 0.5));
}

.text-glow-teal {
  text-shadow: 0 0 8px rgba(45, 212, 191, 0.7);
}

/* ===== SVG 遮罩 (加载面板用) ===== */
.masked-loading {
  mask: url(#cat-mask);
  -webkit-mask: url(#cat-mask);
}

/* ===== 装饰漂浮 ===== */
@keyframes cyberFloat {
  0%,
  100% {
    transform: translateY(0px) rotate(0deg);
  }
  50% {
    transform: translateY(-8px) rotate(3deg);
  }
}

.animate-float {
  animation: cyberFloat 4s ease-in-out infinite;
}

/* 确保根容器覆盖全屏 */
.loading-transition-root {
  position: fixed;
  inset: 0;
  z-index: 9999;
  pointer-events: none;
}

.loading-transition-root > * {
  pointer-events: auto;
}
</style>
