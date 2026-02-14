<template>
  <div class="fireworks-container">
    <canvas ref="trailsCanvas" class="trails-canvas"></canvas>
    <canvas ref="mainCanvas" class="main-canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, nextTick, computed } from 'vue'

// Constants
const MAX_WIDTH = 7680
const MAX_HEIGHT = 4320
const GRAVITY = 0.9
const PI_2 = Math.PI * 2
const PI_HALF = Math.PI * 0.5
const TARGET_FPS = 60
const FRAME_DURATION = 1000 / TARGET_FPS

// Colors
const COLOR = {
  Red: '#ff0043',
  Green: '#14fc56',
  Blue: '#1e7fff',
  Purple: '#e60aff',
  Gold: '#ffbf36',
  White: '#ffffff',
} as const
const INVISIBLE = '_INVISIBLE_'
const COLOR_NAMES = Object.keys(COLOR)
const COLOR_CODES = COLOR_NAMES.map((colorName) => COLOR[colorName as keyof typeof COLOR])
const COLOR_CODES_W_INVIS = [...COLOR_CODES, INVISIBLE]

// Canvas refs
const trailsCanvas = ref<HTMLCanvasElement>()
const mainCanvas = ref<HTMLCanvasElement>()

// State
let stageW = 0
let stageH = 0
let simSpeed = 1
let currentFrame = 0
let activePointerCount = 0
let isUpdatingSpeed = false
let speedBarOpacity = 0
let isPaused = false
let isRunning = false
let isVisible = true

// Performance control
let lastFrameTime = 0

// Animation frame
let animationId: number
let autoLaunchInterval: number

// Stars and particles
interface StarInstance {
  visible: boolean
  heavy: boolean
  x: number
  y: number
  prevX: number
  prevY: number
  color: string
  speedX: number
  speedY: number
  life: number
  fullLife: number
  spinAngle: number
  spinSpeed: number
  spinRadius: number
  sparkFreq: number
  sparkSpeed: number
  sparkTimer: number
  sparkColor: string
  sparkLife: number
  sparkLifeVariation: number
  strobe: boolean
  updateFrame: number
  onDeath?: (instance: StarInstance) => void
  secondColor?: string
  transitionTime: number
  colorChanged: boolean
}

interface SparkInstance {
  x: number
  y: number
  prevX: number
  prevY: number
  color: string
  speedX: number
  speedY: number
  life: number
}

interface BurstFlashInstance {
  x: number
  y: number
  radius: number
}

const Star = {
  active: createParticleCollection(),
  _pool: [] as StarInstance[],
  drawWidth: 3,
  airDrag: 0.98,
  airDragHeavy: 0.992,
  _new(): StarInstance {
    return {
      visible: true,
      heavy: false,
      x: 0,
      y: 0,
      prevX: 0,
      prevY: 0,
      color: '',
      speedX: 0,
      speedY: 0,
      life: 0,
      fullLife: 0,
      spinAngle: 0,
      spinSpeed: 0.8,
      spinRadius: 0,
      sparkFreq: 0,
      sparkSpeed: 1,
      sparkTimer: 0,
      sparkColor: '',
      sparkLife: 750,
      sparkLifeVariation: 0.25,
      strobe: false,
      updateFrame: 0,
      secondColor: undefined,
      transitionTime: 0,
      colorChanged: false,
    }
  },
  add(
    x: number,
    y: number,
    color: string,
    angle: number,
    speed: number,
    life: number,
    speedOffX?: number,
    speedOffY?: number,
  ): StarInstance {
    const instance = this._pool.pop() || this._new()
    instance.visible = true
    instance.heavy = false
    instance.x = x
    instance.y = y
    instance.prevX = x
    instance.prevY = y
    instance.color = color
    instance.speedX = Math.sin(angle) * speed + (speedOffX || 0)
    instance.speedY = Math.cos(angle) * speed + (speedOffY || 0)
    instance.life = life
    instance.fullLife = life
    instance.spinAngle = Math.random() * PI_2
    instance.spinSpeed = 0.8
    instance.spinRadius = 0
    instance.sparkFreq = 0
    instance.sparkSpeed = 1
    instance.sparkTimer = 0
    instance.sparkColor = color
    instance.sparkLife = 750
    instance.sparkLifeVariation = 0.25
    instance.strobe = false
    instance.updateFrame = 0
    instance.secondColor = undefined
    instance.transitionTime = 0
    instance.colorChanged = false
    this.active[color]!.push(instance)
    return instance
  },
  returnInstance(instance: StarInstance) {
    instance.onDeath && instance.onDeath(instance)
    instance.onDeath = undefined
    instance.secondColor = undefined
    instance.transitionTime = 0
    instance.colorChanged = false
    this._pool.push(instance)
  },
}

const Spark = {
  active: createParticleCollection(),
  _pool: [] as SparkInstance[],
  drawWidth: 2.5,
  airDrag: 0.95,
  _new(): SparkInstance {
    return {
      x: 0,
      y: 0,
      prevX: 0,
      prevY: 0,
      color: '',
      speedX: 0,
      speedY: 0,
      life: 0,
    }
  },
  add(
    x: number,
    y: number,
    color: string,
    angle: number,
    speed: number,
    life: number,
  ): SparkInstance {
    const instance = this._pool.pop() || this._new()
    instance.x = x
    instance.y = y
    instance.prevX = x
    instance.prevY = y
    instance.color = color
    instance.speedX = Math.sin(angle) * speed
    instance.speedY = Math.cos(angle) * speed
    instance.life = life
    this.active[color]!.push(instance)
    return instance
  },
  returnInstance(instance: SparkInstance) {
    this._pool.push(instance)
  },
}

const BurstFlash = {
  active: [] as BurstFlashInstance[],
  _pool: [] as BurstFlashInstance[],
  _new(): BurstFlashInstance {
    return {
      x: 0,
      y: 0,
      radius: 0,
    }
  },
  add(x: number, y: number, radius: number): BurstFlashInstance {
    const instance = this._pool.pop() || this._new()
    instance.x = x
    instance.y = y
    instance.radius = radius
    this.active.push(instance)
    return instance
  },
  returnInstance(instance: BurstFlashInstance) {
    this._pool.push(instance)
  },
}

// Helper functions
function createParticleCollection() {
  const collection: Record<string, any[]> = {}
  COLOR_CODES_W_INVIS.forEach((color) => {
    collection[color] = []
  })
  return collection
}

function randomColor() {
  return COLOR_CODES[(Math.random() * COLOR_CODES.length) | 0]
}

function createParticleArc(
  start: number,
  arcLength: number,
  count: number,
  randomness: number,
  particleFactory: (angle: number) => void,
) {
  const angleDelta = arcLength / count
  const end = start + arcLength - angleDelta * 0.5

  if (end > start) {
    for (let angle = start; angle < end; angle = angle + angleDelta) {
      particleFactory(angle + Math.random() * angleDelta * randomness)
    }
  } else {
    for (let angle = start; angle > end; angle = angle + angleDelta) {
      particleFactory(angle + Math.random() * angleDelta * randomness)
    }
  }
}

function createBurst(
  count: number,
  particleFactory: (angle: number, ringSize: number) => void,
  startAngle = 0,
  arcLength = PI_2,
) {
  const R = 0.5 * Math.sqrt(count / Math.PI)
  const C = 2 * R * Math.PI
  const C_HALF = C / 2

  for (let i = 0; i <= C_HALF; i++) {
    const ringAngle = (i / C_HALF) * PI_HALF
    const ringSize = Math.cos(ringAngle)
    const partsPerFullRing = C * ringSize
    const partsPerArc = partsPerFullRing * (arcLength / PI_2)
    const angleInc = PI_2 / partsPerFullRing
    const angleOffset = Math.random() * angleInc + startAngle
    const maxRandomAngleOffset = angleInc * 0.33

    for (let i = 0; i < partsPerArc; i++) {
      const randomAngleOffset = Math.random() * maxRandomAngleOffset
      let angle = angleInc * i + angleOffset + randomAngleOffset
      particleFactory(angle, ringSize)
    }
  }
}

// Shell types
function crysanthemumShell(size: number) {
  const glitter = Math.random() < 0.25
  const singleColor = Math.random() < 0.72
  const color = singleColor ? randomColor() : [randomColor(), randomColor()]
  const pistil = singleColor && Math.random() < 0.42
  const pistilColor = pistil && makePistilColor(color as string)
  const secondColor =
    singleColor && (Math.random() < 0.2 || color === COLOR.White)
      ? pistilColor || randomColor()
      : null
  const streamers = !pistil && color !== COLOR.White && Math.random() < 0.42
  const starDensity = glitter ? 1.1 : 1.25

  return {
    shellSize: size,
    spreadSize: 300 + size * 100,
    starLife: 900 + size * 200,
    starDensity,
    color,
    secondColor,
    glitter: glitter ? 'light' : '',
    glitterColor: Math.random() < 0.5 ? COLOR.Gold : COLOR.White,
    pistil,
    pistilColor,
    streamers,
  }
}

function makePistilColor(shellColor: string | string[]) {
  return shellColor === COLOR.White || shellColor === COLOR.Gold
    ? randomColor()
    : Math.random() < 0.5
      ? COLOR.Gold
      : COLOR.White
}

// Shell class
interface ShellOptions {
  starLifeVariation?: number
  color?: string | string[]
  glitterColor?: string
  starDensity?: number
  spreadSize: number
  starLife: number
  starCount?: number
  glitter?: string
  secondColor?: string
  strobe?: boolean
  strobeColor?: string
  horsetail?: boolean
  ring?: boolean
}

// Shell class
class Shell {
  starLifeVariation: number
  color?: string | string[]
  glitterColor?: string
  starCount: number
  spreadSize?: number
  starLife?: number
  glitter?: string
  secondColor?: string
  strobe?: boolean
  strobeColor?: string
  horsetail?: boolean
  ring?: boolean

  constructor(options: ShellOptions) {
    Object.assign(this, options)
    this.starLifeVariation = options.starLifeVariation || 0.125
    this.color = options.color || randomColor()
    this.glitterColor =
      options.glitterColor || (typeof this.color === 'string' ? this.color : randomColor())

    if (!this!.starCount) {
      const density = options.starDensity || 1
      const scaledSize = (this.spreadSize || 300) / 54
      this.starCount = Math.max(6, scaledSize * scaledSize * density)
    }
  }

  launch(position: number, launchHeight: number) {
    const width = stageW
    const height = stageH
    const hpad = 60
    const vpad = 50
    const minHeightPercent = 0.45
    const minHeight = height - height * minHeightPercent

    const launchX = position * (width - hpad * 2) + hpad
    const launchY = height
    const burstY = minHeight - launchHeight * (minHeight - vpad)
    const launchDistance = launchY - burstY
    const launchVelocity = Math.pow(launchDistance * 0.04, 0.64)

    const comet = Star.add(
      launchX,
      launchY,
      typeof this.color === 'string' && this.color !== 'random' ? this.color : COLOR.White,
      Math.PI,
      launchVelocity * (this.horsetail ? 1.2 : 1),
      launchVelocity * (this.horsetail ? 100 : 400),
    )

    comet.heavy = true
    comet.spinRadius = Math.random() * (0.85 - 0.32) + 0.32
    comet.sparkFreq = 32
    comet.sparkLife = 320
    comet.sparkLifeVariation = 3

    comet.onDeath = () => this.burst(comet.x, comet.y)
  }

  burst(x: number, y: number) {
    const speed = (this.spreadSize || 0) / 96
    let color = this.color
    let sparkFreq = 0
    let sparkSpeed = 0
    let sparkLife = 0
    let sparkLifeVariation = 0.25

    if (this.glitter === 'light') {
      sparkFreq = 400
      sparkSpeed = 0.3
      sparkLife = 600
      sparkLifeVariation = 2
    } else if (this.glitter === 'medium') {
      sparkFreq = 200
      sparkSpeed = 0.44
      sparkLife = 1400
      sparkLifeVariation = 2
    } else if (this.glitter === 'heavy') {
      sparkFreq = 80
      sparkSpeed = 0.8
      sparkLife = 2800
      sparkLifeVariation = 2
    }

    sparkFreq = sparkFreq / 1 // quality

    const starFactory = (angle: number, speedMult: number) => {
      const standardInitialSpeed = (this.spreadSize || 0) / 1800
      const star = Star.add(
        x,
        y,
        (color as string) || (randomColor() as string),
        angle,
        speedMult * speed,
        (this.starLife as number) +
          Math.random() * (this.starLife as number) * this.starLifeVariation,
        this.horsetail ? 0 : 0,
        this.horsetail ? 0 : -standardInitialSpeed,
      )

      if (this.secondColor) {
        star.transitionTime = (this.starLife as number) * (Math.random() * 0.05 + 0.32)
        star.secondColor = this.secondColor
      }

      if (this.strobe) {
        star.transitionTime = (this.starLife as number) * (Math.random() * 0.08 + 0.46)
        star.strobe = true
        if (this.strobeColor) {
          star.secondColor = this.strobeColor
        }
      }

      if (this.glitter) {
        star.sparkFreq = sparkFreq
        star.sparkSpeed = sparkSpeed
        star.sparkLife = sparkLife
        star.sparkLifeVariation = sparkLifeVariation
        star.sparkColor = this.glitterColor as string
        star.sparkTimer = Math.random() * star.sparkFreq
      }
    }

    if (typeof this.color === 'string') {
      if (this.color === 'random') {
        color = randomColor()
      } else {                    
        color = this.color
      }

      if (this.ring) {
        const ringStartAngle = Math.random() * Math.PI
        const ringSquash = Math.pow(Math.random(), 2) * 0.85 + 0.15

        createParticleArc(0, PI_2, this.starCount, 0, (angle) => {
          const initSpeedX = Math.sin(angle) * speed * ringSquash
          const initSpeedY = Math.cos(angle) * speed
          const newSpeed = Math.sqrt(initSpeedX * initSpeedX + initSpeedY * initSpeedY)
          const newAngle = Math.atan2(initSpeedY, initSpeedX) + ringStartAngle
          const star = Star.add(
            x,
            y,
            color as string,
            newAngle,
            newSpeed,
            (this.starLife as number) +
              Math.random() * (this.starLife as number) * this.starLifeVariation,
          )

          if (this.glitter) {
            star.sparkFreq = sparkFreq
            star.sparkSpeed = sparkSpeed
            star.sparkLife = sparkLife
            star.sparkLifeVariation = sparkLifeVariation
            star.sparkColor = this.glitterColor as string
            star.sparkTimer = Math.random() * star.sparkFreq
          }
        })
      } else {
        createBurst(this.starCount, starFactory)
      }
    } else if (Array.isArray(this.color)) {
      if (Math.random() < 0.5) {
        const start = Math.random() * Math.PI
        const start2 = start + Math.PI
        const arc = Math.PI
        color = this.color[0]
        createBurst(this.starCount / 2, starFactory, start, arc)
        color = this.color[1]
        createBurst(this.starCount / 2, starFactory, start2, arc)
      } else {
        color = this.color[0]
        createBurst(this.starCount / 2, starFactory)
        color = this.color[1]
        createBurst(this.starCount / 2, starFactory)
      }
    }

    BurstFlash.add(x, y, (this.spreadSize as number) / 4)
  }
}

// Event handlers
function handleResize() {
  const w = window.innerWidth
  const h = window.innerHeight
  const containerW = Math.min(w, MAX_WIDTH)
  const containerH = w <= 420 ? h : Math.min(h, MAX_HEIGHT)

  stageW = containerW
  stageH = containerH

  if (trailsCanvas.value && mainCanvas.value) {
    trailsCanvas.value.width = containerW
    trailsCanvas.value.height = containerH
    mainCanvas.value.width = containerW
    mainCanvas.value.height = containerH
  }
}

function handlePointerStart(event: PointerEvent) {
  activePointerCount++

  if (!isRunning) return

  if (updateSpeedFromEvent(event)) {
    isUpdatingSpeed = true
  } else {
    launchRandomShell(event.clientX, event.clientY)
  }
}

function handlePointerEnd() {
  activePointerCount--
  isUpdatingSpeed = false
}

function handlePointerMove(event: PointerEvent) {
  if (!isRunning) return
  if (isUpdatingSpeed) {
    updateSpeedFromEvent(event)
  }
}

function updateSpeedFromEvent(event: PointerEvent) {
  if (isUpdatingSpeed || event.clientY >= (mainCanvas.value?.height || 0) - 44) {
    const edge = 16
    const newSpeed = (event.clientX - edge) / ((mainCanvas.value?.width || 0) - edge * 2)
    simSpeed = Math.min(Math.max(newSpeed, 0), 1)
    speedBarOpacity = 1
    return true
  }
  return false
}

function launchRandomShell(x: number, y: number) {
  const audio = new Audio('/audio/fireworks.mp3')
  audio.volume = 0.5
  audio.play().catch((error) => {
    console.log('Audio playback failed:', error)
  })

  const shell = new Shell(crysanthemumShell(3) as ShellOptions)
  const w = stageW
  const h = stageH
  const position = x / w
  const launchHeight = 1 - y / h
  shell.launch(position, launchHeight)
}

function handleVisibilityChange() {
  isVisible = !document.hidden
  if (!isVisible) {
    // When tab becomes hidden, pause the simulation and clean up particles
    isPaused = true
    // Clear all active particles to prevent accumulation
    clearAllParticles()
  } else {
    // When tab becomes visible again, resume
    isPaused = false
  }
}

function clearAllParticles() {
  // Clear all stars
  COLOR_CODES_W_INVIS.forEach((color) => {
    const stars = Star.active[color]
    while (stars!.length > 0) {
      const star = stars!.pop()!
      Star.returnInstance(star)
    }
  })

  // Clear all sparks
  COLOR_CODES_W_INVIS.forEach((color) => {
    const sparks = Spark.active[color]
    while (sparks!.length > 0) {
      const spark = sparks!.pop()!
      Spark.returnInstance(spark)
    }
  })

  // Clear all burst flashes
  while (BurstFlash.active.length > 0) {
    const flash = BurstFlash.active.pop()!
    BurstFlash.returnInstance(flash)
  }
}

function updateGlobals() {
  currentFrame++

  if (!isUpdatingSpeed) {
    speedBarOpacity -= 1 / 30
    if (speedBarOpacity < 0) speedBarOpacity = 0
  }
}

function update() {
  if (!isRunning) return

  const width = stageW
  const height = stageH
  const timeStep = 16.6667 * simSpeed
  const speed = simSpeed
  const starDrag = 1 - (1 - Star.airDrag) * speed
  const starDragHeavy = 1 - (1 - Star.airDragHeavy) * speed
  const sparkDrag = 1 - (1 - Spark.airDrag) * speed
  const gAcc = (timeStep / 1000) * GRAVITY

  updateGlobals()

  COLOR_CODES_W_INVIS.forEach((color) => {
    const stars = Star.active[color]
    for (let i = stars!.length - 1; i >= 0; i--) {
      const star = stars![i]
      if (star.updateFrame === currentFrame) continue
      star.updateFrame = currentFrame

      star.life -= timeStep
      if (star.life <= 0) {
        stars!.splice(i, 1)
        Star.returnInstance(star)
      } else {
        const burnRate = Math.pow(star.life / star.fullLife, 0.5)
        const burnRateInverse = 1 - burnRate

        star.prevX = star.x
        star.prevY = star.y
        star.x += star.speedX * speed
        star.y += star.speedY * speed

        if (!star.heavy) {
          star.speedX *= starDrag
          star.speedY *= starDrag
        } else {
          star.speedX *= starDragHeavy
          star.speedY *= starDragHeavy
        }

        star.speedY += gAcc

        if (star.sparkFreq) {
          star.sparkTimer -= timeStep
          while (star.sparkTimer < 0) {
            star.sparkTimer += star.sparkFreq * 0.75 + star.sparkFreq * burnRateInverse * 4
            Spark.add(
              star.x,
              star.y,
              star.sparkColor,
              Math.random() * PI_2,
              Math.random() * star.sparkSpeed * burnRate,
              star.sparkLife * 0.8 + Math.random() * star.sparkLifeVariation * star.sparkLife,
            )
          }
        }

        if (star.life < star.transitionTime) {
          if (star.secondColor && !star.colorChanged) {
            star.colorChanged = true
            star.color = star.secondColor
            stars!.splice(i, 1)
            Star.active[star.secondColor]!.push(star)
            if (star.secondColor === INVISIBLE) {
              star.sparkFreq = 0
            }
          }

          if (star.strobe) {
            star.visible = Math.floor(star.life / star.strobeFreq) % 3 === 0
          }
        }
      }
    }

    const sparks = Spark.active[color]
    for (let i = sparks!.length - 1; i >= 0; i--) {
      const spark = sparks![i]
      spark.life -= timeStep
      if (spark.life <= 0) {
        sparks!.splice(i, 1)
        Spark.returnInstance(spark)
      } else {
        spark.prevX = spark.x
        spark.prevY = spark.y
        spark.x += spark.speedX * speed
        spark.y += spark.speedY * speed
        spark.speedX *= sparkDrag
        spark.speedY *= sparkDrag
        spark.speedY += gAcc
      }
    }
  })

  render()
}

function render() {
  const trailsCtx = trailsCanvas.value?.getContext('2d')
  const mainCtx = mainCanvas.value?.getContext('2d')

  if (!trailsCtx || !mainCtx) return

  const width = stageW
  const height = stageH

  // Clear the canvas without filling with black
  trailsCtx.clearRect(0, 0, width, height)
  mainCtx.clearRect(0, 0, width, height)

  trailsCtx.globalCompositeOperation = 'lighten'

  trailsCtx.lineWidth = Star.drawWidth
  trailsCtx.lineCap = 'round'
  mainCtx.strokeStyle = '#fff'
  mainCtx.lineWidth = 1
  mainCtx.beginPath()

  COLOR_CODES.forEach((color) => {
    const stars = Star.active[color]
    trailsCtx.strokeStyle = color
    trailsCtx.beginPath()
    stars!.forEach((star) => {
      if (star.visible) {
        trailsCtx.moveTo(star.x, star.y)
        trailsCtx.lineTo(star.prevX, star.prevY)
        mainCtx.moveTo(star.x, star.y)
        mainCtx.lineTo(star.x - star.speedX * 1.6, star.y - star.speedY * 1.6)
      }
    })
    trailsCtx.stroke()
  })
  mainCtx.stroke()

  trailsCtx.lineWidth = Spark.drawWidth
  trailsCtx.lineCap = 'butt'
  COLOR_CODES.forEach((color) => {
    const sparks = Spark.active[color]
    trailsCtx.strokeStyle = color
    trailsCtx.beginPath()
    sparks!.forEach((spark) => {
      trailsCtx.moveTo(spark.x, spark.y)
      trailsCtx.lineTo(spark.prevX, spark.prevY)
    })
    trailsCtx.stroke()
  })

  if (speedBarOpacity) {
    const speedBarHeight = 6
    mainCtx.globalAlpha = speedBarOpacity
    mainCtx.fillStyle = COLOR.Blue
    mainCtx.fillRect(0, height - speedBarHeight, width * simSpeed, speedBarHeight)
    mainCtx.globalAlpha = 1
  }
}

// Lifecycle
onMounted(async () => {
  await nextTick()
  handleResize()

  window.addEventListener('resize', handleResize)
  window.addEventListener('pointerdown', handlePointerStart)
  window.addEventListener('pointerup', handlePointerEnd)
  window.addEventListener('pointermove', handlePointerMove)
  document.addEventListener('visibilitychange', handleVisibilityChange)

  isRunning = true
  isPaused = false
  isVisible = !document.hidden

  // Start with some random fireworks
  autoLaunchInterval = setInterval(() => {
    if (isRunning && !isPaused && isVisible) {
      const fireworkCount = Math.random() < 0.7 ? 1 : Math.floor(Math.random() * 5) + 1
      for (let i = 0; i < fireworkCount; i++) {
        launchRandomShell(Math.random() * stageW, Math.random() * stageH * 0.5)
      }
    }
  }, 3000)

  // Start animation loop
  function loop(timestamp: number) {
    // Frame rate control
    const deltaTime = timestamp - lastFrameTime

    if (deltaTime >= FRAME_DURATION && isVisible) {
      update()
      lastFrameTime = timestamp - (deltaTime % FRAME_DURATION)
    }

    animationId = requestAnimationFrame(loop)
  }
  lastFrameTime = performance.now()
  animationId = requestAnimationFrame(loop)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  window.removeEventListener('pointerdown', handlePointerStart)
  window.removeEventListener('pointerup', handlePointerEnd)
  window.removeEventListener('pointermove', handlePointerMove)
  document.removeEventListener('visibilitychange', handleVisibilityChange)
  cancelAnimationFrame(animationId)
  clearInterval(autoLaunchInterval)
})
</script>

<style scoped>
.fireworks-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: -3;
  pointer-events: none;
}

.trails-canvas,
.main-canvas {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
}
</style>
