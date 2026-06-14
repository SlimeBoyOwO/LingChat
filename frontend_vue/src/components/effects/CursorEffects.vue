<template>
  <div class="cursor-effects-container">
    <canvas ref="canvasRef" class="cursor-trail-canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { useSettingsStore } from '../../stores/modules/settings'

const canvasRef = ref<HTMLCanvasElement | null>(null)
const settingsStore = useSettingsStore()

const FILLED_CIRCLE_CFG = { rAddRate: 26, maxLife: 16 }
const RINGS_ANIM_CFG = {
  rsList: [0, 0.08, 0.1],
  rRoundRateList: [0, 1, 1.5, 2],
  len: 1.1 * Math.PI,
  maxLife: 23,
  segNum: 10,
  minW: 0.4,
  maxW: 3.3,
  lenStopAddPoint: 0.1,
  lenStartDimPoint: 0.4,
}
const CREATE_CLICK_CFG = {
  rings: {
    rsList: [0, 0.03, 0.06],
    rRoundRateList: [0, 1, 1.5, 2],
    len: 1.1 * Math.PI,
  },
  sparksCount: 4,
}

function ringsEndColorFromRgb(rgbString: string) {
  return rgbString.split(',').map(Number).map((n) => (n + 255 * 2) / 3)
}

class MouseSpark {
  mainCanvas: HTMLCanvasElement
  mainCtx: CanvasRenderingContext2D | null = null
  bufferCanvas: HTMLCanvasElement | null = null
  bufferCtx: CanvasRenderingContext2D | null = null

  color: string
  scale: number
  opacity: number
  trailSpeed: number
  clickSpeed: number
  maxTrail: number

  sparksPool: any[] = []
  wavesPool: any[] = []

  waves: any[] = []
  sparks: any[] = []
  trail: any[] = []
  isDown: boolean = false
  lastPos: { x: number, y: number } | null = null
  lastMouseTime: number = 0
  MOUSE_THROTTLE: number = 16
  baseFrameMs: number = 1000 / 60
  maxDeltaMs: number = 100
  lastFrameTime: number = performance.now()
  lastDrawTime: number = 0
  targetFPS: number = 60
  frameInterval: number = 1000 / 60
  dpr: number = 1
  cssWidth: number = 1
  cssHeight: number = 1
  previousDirtyRects: any[] = []
  forceFullRedraw: boolean = true

  ringsStartColor: number[] = [250, 252, 252]
  ringsEndColor: number[]

  animationId: number | null = null

  constructor(canvas: HTMLCanvasElement, opts: any = {}) {
    this.mainCanvas = canvas
    this.color = opts.color || '45,175,255'
    this.scale = opts.scale || 1.5
    this.opacity = opts.opacity || 1.0
    this.trailSpeed = opts.trailSpeed != null ? opts.trailSpeed : (opts.speed || 1.0)
    this.clickSpeed = opts.clickSpeed != null ? opts.clickSpeed : (opts.speed || 1.0)
    this.maxTrail = opts.maxTrail || 36

    this.ringsEndColor = ringsEndColorFromRgb(this.color)

    this.bindHandlers()
    this.initCanvas()
    this.bindEvents()
    this.animationId = requestAnimationFrame((now) => this.animationLoops(now))
  }

  onMouseDown: (e: MouseEvent) => void = () => {}
  onMouseMove: (e: MouseEvent) => void = () => {}
  onMouseUp: (e: MouseEvent) => void = () => {}
  onResize: () => void = () => {}

  bindHandlers() {
    const getPos = (e: MouseEvent) => ({ x: e.clientX, y: e.clientY })
    const dist = (a: any, b: any) => Math.hypot(a.x - b.x, a.y - b.y)

    this.onMouseDown = (e: MouseEvent) => {
      if (!settingsStore.clickAnimationEnabled) return
      this.isDown = true
      this.lastPos = getPos(e)
      this.createEffects(this.lastPos.x, this.lastPos.y)
    }

    this.onMouseMove = (e: MouseEvent) => {
      if (!settingsStore.globalMouseTrailEnabled) return
      const p = getPos(e)
      const prev = this.lastPos
      if (!prev) {
        this.lastPos = p
        return
      }

      const now = performance.now()
      if (now - this.lastMouseTime < this.MOUSE_THROTTLE) {
        return
      }
      this.lastMouseTime = now

      if (dist(p, prev) > 2) {
        this.trail.push({ x: p.x, y: p.y, alpha: 1.0 })
        if (this.trail.length > this.maxTrail * 1.5) {
          this.trail.splice(0, this.trail.length - this.maxTrail)
        }
      }
      this.lastPos = p
    }

    this.onMouseUp = () => {
      this.isDown = false
    }

    this.onResize = () => {
      this.resize()
    }
  }

  bindEvents() {
    window.addEventListener("mousedown", this.onMouseDown)
    window.addEventListener("mousemove", this.onMouseMove)
    window.addEventListener("mouseup", this.onMouseUp)
    window.addEventListener("resize", this.onResize)
  }

  destroy() {
    window.removeEventListener("mousedown", this.onMouseDown)
    window.removeEventListener("mousemove", this.onMouseMove)
    window.removeEventListener("mouseup", this.onMouseUp)
    window.removeEventListener("resize", this.onResize)
    if (this.animationId) {
      cancelAnimationFrame(this.animationId)
    }
  }

  initCanvas() {
    this.mainCtx = this.mainCanvas.getContext("2d")
    this.bufferCanvas = document.createElement("canvas")
    this.bufferCtx = this.bufferCanvas.getContext("2d")
    this.resize()
  }

  alpha(value: number) {
    return Math.max(0, Math.min(1, value * this.opacity))
  }

  resize() {
    const dpr = window.devicePixelRatio || 1
    const cssWidth = Math.max(1, window.innerWidth)
    const cssHeight = Math.max(1, window.innerHeight)
    const w = Math.max(1, Math.floor(cssWidth * dpr))
    const h = Math.max(1, Math.floor(cssHeight * dpr))

    this.dpr = dpr
    this.cssWidth = cssWidth
    this.cssHeight = cssHeight
    this.mainCanvas.width = w
    this.mainCanvas.height = h
    if (this.bufferCanvas && this.bufferCtx) {
      this.bufferCanvas.width = w
      this.bufferCanvas.height = h
      this.bufferCtx.setTransform(dpr, 0, 0, dpr, 0, 0)
    }
    this.previousDirtyRects = []
    this.forceFullRedraw = true
  }

  createEffects(x: number, y: number) {
    const rc = CREATE_CLICK_CFG.rings
    const sparksCount = CREATE_CLICK_CFG.sparksCount

    let wave: any
    if (this.wavesPool.length > 0) {
      wave = this.wavesPool.pop()
    } else {
      wave = {}
    }
    if (!wave.ring) wave.ring = { segs: [] }

    wave.x = x
    wave.y = y
    wave.r = 0
    wave.life = 0
    wave.ring.ang = Math.random() * Math.PI * 2
    wave.ring.rs = rc.rsList[Math.floor(Math.random() * rc.rsList.length)]
    wave.ring.segs[0] = {
      off: 0,
      len: rc.len,
      rRoundRate: rc.rRoundRateList[Math.floor(Math.random() * rc.rRoundRateList.length)],
    }
    wave.ring.segs[1] = {
      off: (Math.random() * 3 - 1.5) * Math.PI,
      len: rc.len,
      rRoundRate: rc.rRoundRateList[Math.floor(Math.random() * rc.rRoundRateList.length)],
    }

    this.waves.push(wave)

    const speedAdjust = this.scale / 1.5
    for (let i = 0; i < sparksCount; i++) {
      const a = Math.random() * Math.PI * 2
      const speed = (4.8 + Math.random() * 2) * speedAdjust

      let spark: any
      if (this.sparksPool.length > 0) {
        spark = this.sparksPool.pop()
      } else {
        spark = {}
      }

      spark.x = x
      spark.y = y
      spark.vx = Math.cos(a) * speed
      spark.vy = Math.sin(a) * speed
      spark.rot = Math.random() * Math.PI * 2
      spark.rs = (Math.random() - 0.5) * 0.28
      spark.s = (4 + Math.random() * 3) * this.scale
      spark.a = 1
      spark.f = 0.9
      spark.fromClick = true
      const t = Math.random()
      const r = Math.round(100 + 155 * t) // 100 到 255
      const g = Math.round(180 + 75 * t)  // 180 到 255
      const b = 255                       // 蓝色通道保持最高
      spark.color = `${r},${g},${b}`
      this.sparks.push(spark)
    }
  }

  _clearBuffer(rect?: any) {
    const ctx = this.bufferCtx
    if (!ctx) return
    if (rect) {
      ctx.clearRect(rect.x, rect.y, rect.w, rect.h)
    } else {
      ctx.clearRect(0, 0, this.cssWidth, this.cssHeight)
    }
  }

  _clearBufferRects(rects: any[]) {
    if (!rects || rects.length === 0) return
    for (const rect of rects) {
      this._clearBuffer(rect)
    }
  }

  _updateTrail(frameScale: number) {
    const ctx = this.bufferCtx
    if (!ctx) return
    
    // --- 更新点透明度 ---
    for (let i = this.trail.length - 1; i >= 0; i--) {
      const p = this.trail[i]
      if (!p) continue
      p.alpha -= 0.025 * frameScale
      if (p.alpha <= 0) {
        this.trail.splice(i, 1)
      }
    }

    if (this.trail.length > this.maxTrail) {
      this.trail.splice(0, this.trail.length - this.maxTrail)
    }

    if (this.trail.length < 2) return

    ctx.save()
    ctx.lineWidth = 3
    ctx.lineCap = 'butt' // 使用 butt 消除连接处的重叠变黑问题
    ctx.lineJoin = 'round'
    ctx.shadowBlur = 10
    ctx.shadowColor = '#87CEFA'

    let startX = this.trail[0]?.x || 0
    let startY = this.trail[0]?.y || 0
    let startAlpha = this.trail[0]?.alpha || 0

    // 分段绘制：使用线性渐变填充每一段，确保颜色过渡平滑
    for (let i = 1; i < this.trail.length - 1; i++) {
      const p = this.trail[i]
      const nextP = this.trail[i + 1]
      if (!p || !nextP) continue
      const xc = (p.x + nextP.x) * 0.5
      const yc = (p.y + nextP.y) * 0.5
      const endAlpha = (p.alpha + nextP.alpha) * 0.5

      ctx.beginPath()
      ctx.moveTo(startX, startY)
      ctx.quadraticCurveTo(p.x, p.y, xc, yc)

      // 创建每一段的渐变
      const gradient = ctx.createLinearGradient(startX, startY, xc, yc)
      gradient.addColorStop(0, `rgba(135, 206, 250, ${startAlpha})`)
      gradient.addColorStop(1, `rgba(135, 206, 250, ${endAlpha})`)
      ctx.strokeStyle = gradient
      ctx.stroke()

      startX = xc
      startY = yc
      startAlpha = endAlpha
    }

    // 连接到最后一个点
    if (this.trail.length > 1) {
      const last = this.trail[this.trail.length - 1]
      if (!last) {
        ctx.restore()
        return
      }
      ctx.beginPath()
      ctx.moveTo(startX, startY)
      ctx.lineTo(last.x, last.y)

      const gradient = ctx.createLinearGradient(startX, startY, last.x, last.y)
      gradient.addColorStop(0, `rgba(135, 206, 250, ${startAlpha})`)
      gradient.addColorStop(1, `rgba(135, 206, 250, ${last.alpha})`)
      ctx.strokeStyle = gradient
      ctx.stroke()

      // 绘制头部圆帽
      ctx.beginPath()
      ctx.arc(last.x, last.y, 1.5, 0, Math.PI * 2)
      ctx.fillStyle = `rgba(135, 206, 250, ${last.alpha})`
      ctx.fill()
    }
    ctx.restore()
  }

  _strokeRingSegment(wx: number, wy: number, radius: number, a0: number, a1: number, lineWidth: number, strokeStyle: string) {
    const ctx = this.bufferCtx
    if (!ctx) return
    ctx.beginPath()
    ctx.arc(wx, wy, radius, a0, a1)
    ctx.lineWidth = lineWidth
    ctx.strokeStyle = strokeStyle
    ctx.stroke()
  }

  _updateWaves(clickFrameScale: number) {
    const filled = FILLED_CIRCLE_CFG
    const rings = RINGS_ANIM_CFG
    const ctx = this.bufferCtx
    if (!ctx) return

    const updateFilledCircle = (w: any, waveProg: number) => {
      w.life += clickFrameScale
      const ease = 1 - Math.pow(1 - waveProg, 3)
      w.r = filled.rAddRate * this.scale * ease
      const alpha = Math.max(0, 1 - waveProg) * 0.25
      if (alpha > 0) {
        ctx.beginPath()
        ctx.arc(w.x, w.y, w.r, 0, Math.PI * 2)
        ctx.fillStyle = `rgba(${this.color},${this.alpha(alpha)})`
        ctx.fill()
      }
    }

    const updateRings = (w: any, ringProg: number) => {
      const getWeightProp = (t: number) => Math.min(2 - Math.abs(4 * (t - 0.5)), 1)
      const ringRgbAt = (rProg: number) => {
        const t = Math.min(1.2 * rProg, 1)
        const r = this.ringsStartColor[0]! * (1 - t) + this.ringsEndColor[0]! * t
        const g = this.ringsStartColor[1]! * (1 - t) + this.ringsEndColor[1]! * t
        const b = this.ringsStartColor[2]! * (1 - t) + this.ringsEndColor[2]! * t
        return [Math.round(r), Math.round(g), Math.round(b)]
      }
      const getAlpha = (rProg: number) => Math.min(1.1 - 0.3 * rProg, 1)

      const r = w.ring
      r.ang -= r.rs * clickFrameScale

      let start = 0
      let end = 0
      let len = 0
      let seg: any

      for (let i = 0; i < 2; i++) {
        seg = r.segs[i]
        const base = r.ang + seg.off

        if (ringProg <= rings.lenStopAddPoint) {
          len = seg.len * (ringProg / rings.lenStopAddPoint)
          end = base + seg.len
          start = end - len
        } else if (ringProg > rings.lenStartDimPoint) {
          len = seg.len * (1 - (ringProg - rings.lenStartDimPoint) / (1 - rings.lenStartDimPoint))
          start = base
          end = start + len
        } else {
          len = seg.len
          start = base
          end = start + len
        }

        const lineWidthMul = Math.min(-0.8 * (ringProg - 0.8) + 1, 1)
        const [rr, gg, bb] = ringRgbAt(ringProg)
        const alphaRing = getAlpha(ringProg)

        for (let k = 0; k < rings.segNum; k++) {
          const t0 = k / rings.segNum
          const t1 = (k + 1) / rings.segNum
          const a0 = start + (end - start) * t0
          const a1 = start + (end - start) * t1

          if (Math.abs(a1 - a0) < 0.01) continue

          const wT = getWeightProp(t0)
          const lw = (rings.minW * (1 - wT) + rings.maxW * wT) * lineWidthMul
          const strokeStyle = `rgba(${rr},${gg},${bb},${alphaRing})`
          const radius = w.r + seg.rRoundRate * this.scale
          this._strokeRingSegment(w.x, w.y, radius, a0, a1, lw, strokeStyle)
        }
      }
    }

    for (let i = this.waves.length - 1; i >= 0; i--) {
      const w = this.waves[i]
      const waveProg = Math.min(w.life / filled.maxLife, 1)
      const ringProg = Math.min(w.life / rings.maxLife, 1)

      updateFilledCircle(w, waveProg)
      updateRings(w, ringProg)

      if (ringProg >= 1 && waveProg >= 1) {
        this.wavesPool.push(this.waves[i])
        this.waves.splice(i, 1)
      }
    }
  }

  _updateSparks(clickFrameScale: number, trailFrameScale: number) {
    const ctx = this.bufferCtx
    if (!ctx) return
    for (let i = this.sparks.length - 1; i >= 0; i--) {
      const s = this.sparks[i]
      const fs = s.fromClick ? clickFrameScale : trailFrameScale
      s.x += s.vx * fs
      s.y += s.vy * fs
      s.vx *= Math.pow(s.f, fs)
      s.vy *= Math.pow(s.f, fs)
      s.rot += s.rs * fs
      s.a -= 0.032 * fs
      if (s.a <= 0) {
        this.sparksPool.push(this.sparks[i])
        this.sparks.splice(i, 1)
        continue
      }

      ctx.save()
      ctx.translate(s.x, s.y)
      ctx.rotate(s.rot)
      ctx.beginPath()
      ctx.moveTo(0, -s.s)
      ctx.lineTo(s.s * 0.6, s.s * 0.6)
      ctx.lineTo(-s.s * 0.6, s.s * 0.6)
      ctx.fillStyle = `rgba(${s.color},${this.alpha(s.a)})`
      ctx.fill()
      ctx.restore()
    }
  }

  _canvasRect() {
    return { x: 0, y: 0, w: this.cssWidth, h: this.cssHeight }
  }

  _clipRect(rect: any) {
    if (!rect) return null
    const x0 = Math.max(0, Math.floor(rect.x))
    const y0 = Math.max(0, Math.floor(rect.y))
    const x1 = Math.min(this.cssWidth, Math.ceil(rect.x + rect.w))
    const y1 = Math.min(this.cssHeight, Math.ceil(rect.y + rect.h))
    if (x1 <= x0 || y1 <= y0) return null
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
  }

  _pointRect(x: number, y: number, padding: number) {
    return { x: x - padding, y: y - padding, w: padding * 2, h: padding * 2 }
  }

  _segmentRect(a: any, b: any, padding: number) {
    const x0 = Math.min(a.x, b.x) - padding
    const y0 = Math.min(a.y, b.y) - padding
    const x1 = Math.max(a.x, b.x) + padding
    const y1 = Math.max(a.y, b.y) + padding
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
  }

  _intersects(a: any, b: any) {
    return a.x <= b.x + b.w && a.x + a.w >= b.x && a.y <= b.y + b.h && a.y + a.h >= b.y
  }

  _unionRect(a: any, b: any) {
    const x0 = Math.min(a.x, b.x)
    const y0 = Math.min(a.y, b.y)
    const x1 = Math.max(a.x + a.w, b.x + b.w)
    const y1 = Math.max(a.y + a.h, b.y + b.h)
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
  }

  _mergeRects(rects: any[]) {
    const merged: any[] = []
    for (const raw of rects) {
      let rect = this._clipRect(raw)
      if (!rect) continue

      for (let i = 0; i < merged.length; i++) {
        if (this._intersects(merged[i], rect)) {
          rect = this._unionRect(merged[i], rect)
          merged.splice(i, 1)
          i = -1
        }
      }
      merged.push(rect)
    }
    return merged
  }

  _getEffectRects() {
    const rects: any[] = []
    const trailPad = 18 * this.scale + 12
    const trailPoints =
      this.lastPos && this.trail.length > 0
        ? this.trail.concat([{ x: this.lastPos.x, y: this.lastPos.y }])
        : this.trail

    if (trailPoints.length === 1) {
      rects.push(this._pointRect(trailPoints[0].x, trailPoints[0].y, trailPad))
    } else {
      for (let i = 0; i < trailPoints.length - 1; i++) {
        rects.push(this._segmentRect(trailPoints[i], trailPoints[i + 1], trailPad))
      }
    }

    const wavePad = 34 * this.scale + RINGS_ANIM_CFG.maxW + 16
    for (const wave of this.waves) {
      const radius = Math.max(wave.r || 0, FILLED_CIRCLE_CFG.rAddRate * this.scale) + wavePad
      rects.push(this._pointRect(wave.x, wave.y, radius))
    }

    const maxFrameScale = this.maxDeltaMs / this.baseFrameMs
    for (const spark of this.sparks) {
      const speed = Math.hypot(spark.vx || 0, spark.vy || 0)
      const speedScale = spark.fromClick ? this.clickSpeed : this.trailSpeed
      const motionPad = speed * maxFrameScale * speedScale
      const sparkPad = Math.max(spark.s || 0, 9 * this.scale) * 2 + motionPad + 12
      rects.push(this._pointRect(spark.x, spark.y, sparkPad))
    }

    return this._mergeRects(rects)
  }

  _getRenderRects() {
    if (this.forceFullRedraw) {
      return [this._canvasRect()]
    }
    return this._mergeRects(this.previousDirtyRects.concat(this._getEffectRects()))
  }

  _clipToRects(ctx: CanvasRenderingContext2D, rects: any[]) {
    ctx.beginPath()
    for (const rect of rects) {
      ctx.rect(rect.x, rect.y, rect.w, rect.h)
    }
    ctx.clip()
  }

  _renderToMain(rects: any[]) {
    const { mainCtx, mainCanvas, bufferCanvas } = this
    if (!mainCtx || !bufferCanvas) return

    if (!rects || rects.length === 0) {
      mainCtx.clearRect(0, 0, mainCanvas.width, mainCanvas.height)
      mainCtx.drawImage(bufferCanvas, 0, 0)
      return
    }

    const dpr = this.dpr || 1
    for (const rect of rects) {
      const sx = Math.max(0, Math.floor(rect.x * dpr))
      const sy = Math.max(0, Math.floor(rect.y * dpr))
      const sw = Math.min(mainCanvas.width - sx, Math.ceil(rect.w * dpr))
      const sh = Math.min(mainCanvas.height - sy, Math.ceil(rect.h * dpr))
      if (sw <= 0 || sh <= 0) {
        continue
      }

      mainCtx.clearRect(sx, sy, sw, sh)
      mainCtx.drawImage(bufferCanvas, sx, sy, sw, sh, sx, sy, sw, sh)
    }
  }

  animationLoops(now: number) {
    if (now - this.lastDrawTime < this.frameInterval) {
      this.animationId = requestAnimationFrame((nextNow) => this.animationLoops(nextNow))
      return
    }
    this.lastDrawTime = now

    const hasWork = this.waves.length > 0 || this.sparks.length > 0 || this.trail.length > 0

    if (!hasWork) {
      this.lastFrameTime = now
      if (this.previousDirtyRects.length > 0) {
        this._clearBufferRects(this.previousDirtyRects)
        this._renderToMain(this.previousDirtyRects)
        this.previousDirtyRects = []
      }
      this.animationId = requestAnimationFrame((nextNow) => this.animationLoops(nextNow))
      return
    }

    const deltaMs = Math.min(now - this.lastFrameTime, this.maxDeltaMs)
    this.lastFrameTime = now
    const baseScale = deltaMs / this.baseFrameMs
    const trailFrameScale = baseScale * this.trailSpeed
    const clickFrameScale = baseScale * this.clickSpeed

    const bctx = this.bufferCtx
    if (!bctx) return

    const renderRects = this._getRenderRects()
    bctx.save()
    this._clipToRects(bctx, renderRects)
    bctx.globalCompositeOperation = "lighter"

    this._clearBufferRects(renderRects)
    this._updateTrail(trailFrameScale)
    this._updateWaves(clickFrameScale)
    this._updateSparks(clickFrameScale, trailFrameScale)

    bctx.globalCompositeOperation = "source-over"
    bctx.restore()

    this._renderToMain(renderRects)
    this.previousDirtyRects = this._getEffectRects()
    this.forceFullRedraw = false

    this.animationId = requestAnimationFrame((nextNow) => this.animationLoops(nextNow))
  }
}

let sparkInstance: MouseSpark | null = null

watch(
  () => settingsStore.globalMouseTrailEnabled,
  (enabled) => {
    if (!enabled && sparkInstance) {
      sparkInstance.trail.length = 0
    }
  },
)

watch(
  () => settingsStore.clickAnimationEnabled,
  (enabled) => {
    if (!enabled && sparkInstance) {
      sparkInstance.sparks.length = 0
      sparkInstance.waves.length = 0
    }
  },
)

onMounted(() => {
  if (canvasRef.value) {
    sparkInstance = new MouseSpark(canvasRef.value)
  }
})

onBeforeUnmount(() => {
  if (sparkInstance) {
    sparkInstance.destroy()
    sparkInstance = null
  }
})
</script>

<style scoped>
.cursor-effects-container {
  pointer-events: none;
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 9999;
}

.cursor-trail-canvas {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 100%;
}
</style>
