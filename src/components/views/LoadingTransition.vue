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
      class="masked-loading fixed inset-0 z-9998 flex flex-col items-center justify-between py-12 px-6 overflow-hidden"
    >
      <!-- 赛博网格背景 -->
      <div class="absolute inset-0 bg-grid opacity-30"></div>
      <!-- 扫描线效果 -->
      <div class="absolute inset-0 scanline pointer-events-none"></div>

      <!-- 四角装饰 -->
      <div
        class="absolute top-4 left-4 w-8 h-8 border-l-2 border-t-2 border-teal-500/30 pointer-events-none"
      ></div>
      <div
        class="absolute top-4 right-4 w-8 h-8 border-r-2 border-t-2 border-teal-500/30 pointer-events-none"
      ></div>
      <div
        class="absolute bottom-4 left-4 w-8 h-8 border-l-2 border-b-2 border-teal-500/30 pointer-events-none"
      ></div>
      <div
        class="absolute bottom-4 right-4 w-8 h-8 border-r-2 border-b-2 border-teal-500/30 pointer-events-none"
      ></div>

      <!-- 浮动粒子（position:absolute 默认 top:0，translateY(100vh)→(-10vh) = 底部→顶部） -->
      <div
        v-for="p in particles"
        :key="p.id"
        class="absolute rounded-full bg-cyan-400/50 glow-cyan animate-particle pointer-events-none"
        :style="{
          left: p.left + '%',
          width: p.size + 'px',
          height: p.size + 'px',
          animationDuration: p.duration + 's',
          animationDelay: p.delay + 's',
        }"
      ></div>
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

          <div
            class="w-[40vh] text-center text-sm font-mono text-cyan-400/70 transition-all duration-300"
          >
            {{ currentStatusText }}
          </div>

          <!-- 进度条 -->
          <div class="w-[60vh] flex items-center space-y-2 mt-4">
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

            <div
              class="absolute -right-12 justify-center items-center text-sm font-mono text-cyan-300/80"
            >
              <span>{{ Math.floor(progress) }}%</span>
            </div>
          </div>

          <div
            class="w-[40vh] flex text-center justify-center gap-12 text-sm font-mono text-cyan-300/80"
          >
            <span class="text-cyan-500/80">{{ randomTip }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { eventQueue } from '@/core/events/event-queue'

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
      try {
        this.ctx = new (window.AudioContext || (window as any).webkitAudioContext)()
      } catch {
        // 浏览器不支持 Web Audio API
      }
    }
    // 某些浏览器需要 resume（autoplay policy）
    if (this.ctx && this.ctx.state === 'suspended') {
      this.ctx.resume()
    }
  },

  playTick() {
    this.init()
    if (this.isMuted || !this.ctx) return
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
    this.init()
    if (this.isMuted || !this.ctx) return
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
    this.init()
    if (this.isMuted || !this.ctx) return
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
    this.init()
    if (this.isMuted || !this.ctx) return
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

// 背景浮动粒子（负延迟 = 直接进入动画中间阶段，无等待期）
interface Particle {
  id: number
  left: number
  size: number
  duration: number
  delay: number
}
const particles: Particle[] = Array.from({ length: 30 }, (_, i) => ({
  id: i,
  left: Math.random() * 100,
  size: 2 + Math.random() * 5,
  duration: 3 + Math.random() * 5,
  delay: -(Math.random() * 6), // 负值：初始即处于动画中途
}))

// ============================================================
//  加载状态台词（在 50% / 70% / 90% 时切换）
// ============================================================
const statusTexts = [
  '正在校准量子猫爪频率...',
  'TCP 握爪中，请不要松开~',
  '心灵链接建立中，请耐心等待...',
  '正在唤醒终端小可爱...',
  '加载猫薄荷驱动中...',
  '同步星际中继站信号...',
  '正在烘焙虚拟小饼干...',
  '调整全息投影参数中...',
  '喵喵喵，马上就好啦~',
  '正在连接喵星服务器...',
  '激光笔定位校准完毕！',
  '检测到大量可爱粒子，收集中...',
]
const currentStatusText = ref(statusTexts[0])

// 记录已触发过的阈值，防止重复切换
const triggeredThresholds = new Set<number>()

function updateStatusText(p: number) {
  const thresholds = [50, 70, 90]
  for (const t of thresholds) {
    if (p >= t && !triggeredThresholds.has(t)) {
      triggeredThresholds.add(t)
      // 从未使用的台词中选
      const unused = statusTexts.filter((s) => s !== currentStatusText.value)
      const pick = unused[Math.floor(Math.random() * unused.length)]
      if (pick) currentStatusText.value = pick
      return
    }
  }
}

// ============================================================
//  随机小贴士（权重归类随机）
// ============================================================
interface TipCategory {
  name: string
  weight: number
  messages: string[]
}

const tipCategories: TipCategory[] = [
  {
    name: '游戏提示',
    weight: 0.4,
    messages: [
      '欢迎使用 LingChat 终端机 (*^▽^*)',
      'LingChat 正在努力加载中... 稍等一下哦',
      '看进度条会变傻，不许看！',
      '你有打开过久坐提醒功能吗？工作党必须哦~',
      '桌宠模式下的代办功能很方便哦，试试看吧~',
      '欢迎向创意工坊投稿自己制作的角色！',
      '你可以在设置中更改角色和背景哦~',
      '你可以为背景添加相关提示词让它可以被感知哦~',
      '你知道羁绊剧情是可以自己做的吗？试试看吧！',
      '通用板块有很多实用的功能，去试试捏',
      '和 Galgame 一样，不要忘记存档！',
      '别忘了打开永久记忆功能呢',
      '为了你的沉浸式体验，可以加自己喜欢的音乐和环境音哦',
      '在背景界面有很多可以设置的部分，个性化你的游戏去吧！',
      '欢迎前往创意工坊投稿你的角色和剧情！',
      'LingChat 的角色表情是通过深度学习推理出情绪的哦，厉害吧！',
      '永久记忆会自动压缩记忆，放心啦~',
      'LingChat 的运行逻辑比你想象中复杂很多呢，快去探索吧！',
      '发现那个照相机了吗？你可以截图给她看东西的哦！',
      '可以用麦克风直接和她语音对话呢',
      '用你的小爪爪可以摸摸她~',
    ],
  },
  {
    name: '求情广告',
    weight: 0.1,
    messages: [
      '给LingChat点点star喵，给LingChat点点star谢谢喵',
      '灌注钦灵喵，灌注钦灵谢谢喵~',
      '钦灵这么可爱，真的不给她点个star吗？',
      '开发者都是学生哦，软件制作不易~',
      '你有注意到致谢吗？这是社区的力量哦！',
      '开发者缺人啦！欢迎想要参与开发的人加入哦！',
      'LingChat 的开源社区的每一个贡献者都很辛苦哦...',
      'LingChat 的 Star 数量超过 1000 了！好耶！',
    ],
  },
  {
    name: '开发者彩蛋',
    weight: 0.2,
    messages: [
      '你知道吗？钦灵本人比AI钦灵更可爱（也更淫荡）',
      '其实风雪并不会写代码，她只是趴在键盘上睡着了，然后恰好对LingChat提交了commit',
      '影空正在被钦灵催情变得越来越淫荡...',
      '正在加载PL的代码... 等等，python 已经被换成 rust 了...',
      '你们看到云小姐了吗？嗯嗯，她没有失踪也没有怎么样的，我们只是想让你知道，她很可爱',
      '喵？喵~ 喵！',
      'uwa是萝莉音，望周知',
      '七毛钱的苹果？好吃，耶！',
      '大饼鸡蛋一听就很好吃的样子，AstroBot 也有他哦！',
      '喵本喵正在努力的学习画画以此变为项目的黑奴...',
      '开发者群最淫荡的七辰正在用户群穿胖次外漏乱跑中',
      '钦灵正在努力的改写有梦当然留下的组件的代码 >A<',
      '波奶很可爱~啵啵~❤',
      '饲养员莱尔正在努力供养开发者群...',
      '诺亚狐和七辰喜欢管理着文档，也喜欢开银趴',
      '魔法少女总督挥动了神奇的魔法棒，让你看到了奇妙的鼠标粒子特效~',
      'Heiyaha正在视奸你的CPU',
      '哦~卷大人，你是一只可爱的猫娘喵~',
      '徒花花可爱爱，红瞳长发令人爱~',
      '晚安喜欢被钦灵用鞭子抽着屁股干活，嘿嘿嘿',
      '远足正在从钦灵疯狂的偷猫图，可恶！',
      '45454，用 10M token 感动了钦灵做安卓版本，快谢谢他',
      '元初是个小男娘，喜欢艾草',
      'RatMan 挖了个地洞偷偷跑了...',
      '琉璃子非常喜欢白丝，我们希望你能知道，之后可以送他一份',
      'Flame 小姐的代码和她一样美丽',
      'Yukito柏海和梦轩一起创造了你看到的美丽的UI！',
      'Matsuko 喜欢被榨奶... 狐狐澪爱看❤',
    ],
  },
]

/** 按权重随机选分类，再从分类中随机选一条消息 */
function pickRandomTip(): string {
  const totalWeight = tipCategories.reduce((sum, c) => sum + c.weight, 0)
  let r = Math.random() * totalWeight
  for (const cat of tipCategories) {
    r -= cat.weight
    if (r <= 0) {
      return cat.messages[Math.floor(Math.random() * cat.messages.length)]
    }
  }
  // fallback
  return tipCategories[0].messages[0]
}

const randomTip = ref(pickRandomTip())

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

  const duration = 500
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
//  进度条模拟算法
// ============================================================
// 约束：
//   - MIN_DISPLAY_MS = 5000  : 最少展示 5s
//   - MAX_TIMEOUT_MS = 15000 : 最晚 15s 强制完成
//   - ACCEL_WINDOW_MS = 1000 : 检测到事件后 1s 内冲至 100%
// 正常曲线：√(t) 减速增长（起步快、后期慢，模拟真实加载）
// 加速曲线：事件到达后线性冲刺，但仍遵守 5s 最短展示

const MIN_DISPLAY_MS = 5000
const MAX_TIMEOUT_MS = 15000
const ACCEL_WINDOW_MS = 1000
const TICK_MS = 50

let progressTimer: ReturnType<typeof setInterval> | null = null
let startTime = 0
let eventDetectedTime = 0

/** 正常减速曲线：√(t) → 0~95% */
function normalCurve(elapsed: number): number {
  const raw = Math.min(elapsed / (MAX_TIMEOUT_MS - ACCEL_WINDOW_MS), 1)
  return Math.sqrt(raw) * 95
}

/** 根据当前状态计算目标进度 (0–100) */
function computeTarget(elapsed: number): number {
  // 硬上限
  if (elapsed >= MAX_TIMEOUT_MS) return 100

  // 检测事件队列
  const hasEvents = eventQueue.getState().queueLength > 0
  if (hasEvents && eventDetectedTime === 0) {
    eventDetectedTime = elapsed
  }

  if (eventDetectedTime > 0) {
    // 加速模式
    if (elapsed < MIN_DISPLAY_MS) {
      // 未到最短展示期：按比例增长，5s 时恰好 99%
      return (elapsed / MIN_DISPLAY_MS) * 99
    }
    // 已过 5s：从检测点起 ACCEL_WINDOW_MS 内线性完成
    const sinceDetect = elapsed - eventDetectedTime
    const progressAtDetect = normalCurve(eventDetectedTime)
    const remaining = 100 - progressAtDetect
    return Math.min(100, progressAtDetect + remaining * Math.min(1, sinceDetect / ACCEL_WINDOW_MS))
  }

  // 正常模式
  return normalCurve(elapsed)
}

function handleTransitionSequence() {
  if (dotTimer) clearInterval(dotTimer)
  isEstablished.value = true
  NekoSynth.playChime()

  // 等 1.2s 让用户看到 "Connection Established" 后开始转场
  setTimeout(() => {
    isPeeking.value = true
    NekoSynth.playPop()
    animatePeek(() => {
      setTimeout(() => {
        isPeeking.value = false
        isUnveiling.value = true
        animateAnticipation(() => {
          NekoSynth.playUnveil()
          animateUnveil(() => {
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
  if (progressTimer) clearInterval(progressTimer)

  startTime = performance.now()
  eventDetectedTime = 0

  progressTimer = setInterval(() => {
    const elapsed = performance.now() - startTime
    const target = computeTarget(elapsed)

    // 指数平滑：避免跳变，视觉上自然连续
    const smoothStep = (target - progress.value) * 0.12
    progress.value = Math.min(100, progress.value + Math.max(0.3, smoothStep))

    updateStatusText(progress.value)

    // tick 音效：进度越低越密集（营造忙碌感），高进度时降低频率
    const tickChance = progress.value < 40 ? 0.55 : progress.value < 80 ? 0.35 : 0.2
    if (Math.random() < tickChance) NekoSynth.playTick()

    if (progress.value >= 99.9) {
      progress.value = 100
      if (progressTimer) clearInterval(progressTimer)
      progressTimer = null
      handleTransitionSequence()
    }
  }, TICK_MS)
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
  if (progressTimer) clearInterval(progressTimer)
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

/* ===== 粒子上升 ===== */
@keyframes particleUp {
  0% {
    transform: translateY(100vh) scale(0.5);
    opacity: 0;
  }
  50% {
    opacity: 0.6;
  }
  100% {
    transform: translateY(-10vh) scale(1.2);
    opacity: 0;
  }
}

.animate-particle {
  animation: particleUp 8s linear infinite backwards;
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
  background-color: #070f15;
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
