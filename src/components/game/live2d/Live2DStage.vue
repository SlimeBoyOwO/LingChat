<template>
  <div
    v-bind="$attrs"
    ref="host"
    class="absolute
      inset-0
      pointer-events-none
      overflow-hidden"
    aria-hidden="true"
  ></div>
  <slot></slot>
</template>

<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { onBeforeUnmount, onMounted, provide, readonly, ref, watch } from 'vue'

import { getLive2dFilePath } from '@/api/services/character'
import { EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import type { GameRole } from '@/stores/modules/game/state'
import {
  resolveLive2dVariant,
  type Live2dMotionBinding,
  type Live2dVariant,
} from '@/types/live2d'
import { areEyesOpen, focusDirection, pointerToStagePoint } from './live2d-interaction'
import { live2dStageContextKey } from './live2d-stage-context'
import { calculatePetLayout } from './live2d-layout'
import { trackMotionLifecycle } from './live2d-motion'
import { loadLive2dRuntime, type Live2dRuntime } from './live2d-runtime'
import {
  configureRuntimeIdle,
  rewriteModelReferences,
  type Live2dModelSource,
} from './model-source'
import { decodeVoiceForLipSync, sampleVoiceAmplitude, type DecodedVoice } from './useLive2dLipSync'

defineOptions({ inheritAttrs: false })

const props = defineProps<{
  roles: GameRole[]
  mode: 'standard' | 'pet'
  activeSpeakerId: number | null
  audioElement: HTMLAudioElement | null
  voiceDataUrl: string
}>()

const emit = defineEmits<{
  activeChange: [roleIds: number[]]
  failedChange: [roleIds: number[]]
}>()

interface CursorPayload {
  x: number
  y: number
}

interface RoleModel {
  roleId: number
  variantName: string
  model: any
  variant: Live2dVariant
  runtimeIdle: Live2dMotionBinding | null
  emotion: string
  requestId: number
  mouthParameterIndex: number
  mouthValue: number
  eyeLeftParameterIndex: number
  eyeRightParameterIndex: number
  eyesOpen: boolean
  focusFrozen: boolean
  reactionSequence: number
  reactionLifecycleCleanup: (() => void) | null
}

const host = ref<HTMLDivElement | null>(null)
let runtime: Live2dRuntime | null = null
let application: any = null
let disposed = false
let syncPromise = Promise.resolve()
let requestSequence = 0
let decodedVoice: DecodedVoice | null = null
let decodeSequence = 0
let resizeObserver: ResizeObserver | null = null
let pointerPosition: { clientX: number; clientY: number } | null = null
let cursorUnlisten: (() => void) | null = null
const models = new Map<number, RoleModel>()
const failedRoleIds = new Set<number>()
const readyRoleIds = ref<ReadonlySet<number>>(new Set())
const unavailableRoleIds = ref<ReadonlySet<number>>(new Set())

provide(live2dStageContextKey, {
  readyRoleIds: readonly(readyRoleIds),
  unavailableRoleIds: readonly(unavailableRoleIds),
})

function emitFailedRoles() {
  const roleIds = [...failedRoleIds]
  unavailableRoleIds.value = new Set(roleIds)
  emit('failedChange', roleIds)
}

function emitActiveRoles() {
  const roleIds = [...models.keys()]
  readyRoleIds.value = new Set(roleIds)
  emit('activeChange', roleIds)
}

function motionBindingEquals(
  left: Live2dMotionBinding | null | undefined,
  right: Live2dMotionBinding | null | undefined,
) {
  if (!left || !right) return left == null && right == null
  return (
    left.group === right.group &&
    left.index === right.index &&
    (left.loop ?? true) === (right.loop ?? true)
  )
}

function mappedEmotion(emotion: string) {
  return EMOTION_CONFIG_EMO[emotion] || '正常'
}

function variantNameFor(role: GameRole): string | null {
  if (!role.live2d) return null
  const clothes = !role.clothesName || role.clothesName === '默认' ? 'default' : role.clothesName
  const mapped = role.live2d.clothes_variants[clothes]
  return mapped || role.live2d.default_variant
}

async function loadModelSource(roleId: number, modelFile: string) {
  const modelPath = await getLive2dFilePath(roleId, modelFile)
  const modelUrl = convertFileSrc(modelPath)
  const response = await fetch(modelUrl)
  if (!response.ok) throw new Error(`Failed to load Live2D settings: HTTP ${response.status}`)
  const source = (await response.json()) as Live2dModelSource
  await rewriteModelReferences(source, modelFile, async (relative) => {
    return convertFileSrc(await getLive2dFilePath(roleId, relative))
  })
  source.url = modelUrl
  return source
}

function destroyApplication() {
  resizeObserver?.disconnect()
  resizeObserver = null
  if (application) {
    application.destroy({ removeView: true, releaseGlobalResources: false }, true)
    application = null
  }
  pointerPosition = null
  runtime = null
}

async function ensureApplication() {
  if (application || !host.value || disposed) return
  runtime = await loadLive2dRuntime()
  const app = new runtime.pixi.Application()
  await app.init({
    resizeTo: host.value,
    preference: 'webgl',
    backgroundAlpha: 0,
    antialias: true,
    autoDensity: true,
    resolution: Math.min(window.devicePixelRatio, props.mode === 'pet' ? 1.5 : 2),
  })
  if (disposed || !host.value) {
    app.destroy({ removeView: true, releaseGlobalResources: false }, true)
    return
  }
  app.canvas.className = 'absolute inset-0 w-full h-full'
  host.value.appendChild(app.canvas)
  app.ticker.speed = 1.35
  app.ticker.add(updateLipSync)
  resizeObserver = new ResizeObserver(() => {
    for (const entry of models.values()) {
      const role = props.roles.find((item) => item.roleId === entry.roleId)
      if (role) applyLayout(entry, role)
    }
  })
  resizeObserver.observe(host.value)
  application = app
}

function findParameterIndex(entry: RoleModel, parameter: string): number {
  const core = entry.model.internalModel.coreModel
  for (let index = 0; index < core.getParameterCount(); index += 1) {
    if (core.getParameterId(index).isEqual(parameter)) return index
  }
  return -1
}

function updateModelFocus(entry: RoleModel) {
  if (entry.focusFrozen) return
  const focusController = entry.model.internalModel.focusController
  // 标准聊天模式：始终直视前方（不跟随鼠标）；仅桌宠模式用指针驱动视线
  if (props.mode !== 'pet') {
    focusController.focus(0, 0)
    return
  }
  // 眨眼/隐藏期间冻结视线目标：不重置回中。引擎的眨眼控制器会在
  // beforeModelUpdate 之前把眼部参数写成闭眼值，此时若走回中分支，
  // 弹簧插值会把瞳孔/头短暂拽向正中，表现为眨眼瞬间“瞬视中间”。
  if (!entry.eyesOpen || !entry.model.visible) return
  if (!pointerPosition || !host.value || !application) {
    focusController.focus(0, 0)
    return
  }
  const point = pointerToStagePoint(
    pointerPosition.clientX,
    pointerPosition.clientY,
    host.value.getBoundingClientRect(),
    application.screen,
  )
  if (point) {
    const anchor = entry.variant.focus_anchor
    if (!anchor || !runtime) {
      entry.model.focus(point.x, point.y)
      return
    }
    const bounds = entry.model.getLocalBounds()
    const localAnchor = new runtime.pixi.Point(
      (bounds.minX ?? bounds.x ?? 0) + bounds.width * anchor.x,
      (bounds.minY ?? bounds.y ?? 0) + bounds.height * anchor.y,
    )
    const worldAnchor = entry.model.toGlobal(localAnchor)
    const direction = focusDirection(point, worldAnchor)
    focusController.focus(direction.x, direction.y)
  }
}

function handlePointerMove(event: PointerEvent) {
  pointerPosition = { clientX: event.clientX, clientY: event.clientY }
}

function destroyModel(model: any) {
  // Textures loaded through Pixi Assets are shared across stages and owned by the global cache.
  model.destroy({ children: true, texture: false, baseTexture: false })
}

function applyLayout(entry: RoleModel, role: GameRole) {
  if (!application || !host.value) return
  const model = entry.model
  const bounds = model.getLocalBounds()
  const width = bounds.width || model.internalModel.width || model.width || 1
  const height = bounds.height || model.internalModel.height || model.height || 1
  if (props.mode === 'pet') {
    const layout = calculatePetLayout(
      application.screen,
      { width, height },
      role.scaleP || 1,
      role.offsetXP || 0,
      role.offsetYP || 0,
    )
    model.anchor.set(layout.anchorX, layout.anchorY)
    model.scale.set(layout.scale)
    model.position.set(layout.x, layout.y)
  } else {
    const index = props.roles.findIndex((item) => item.roleId === role.roleId)
    const count = props.roles.length
    const xPercent = index < 0 ? 0.5 : (index + 1) / (count + 1)
    const roleScale = role.scale || 1
    const baseScale = application.screen.height / height
    model.anchor.set(0.5, 1)
    model.scale.set(baseScale * roleScale)
    model.position.set(
      application.screen.width * xPercent + (role.offsetX || 0),
      application.screen.height * roleScale + (role.offsetY || 0),
    )
  }
  model.visible = role.show
}

function startIdle(entry: RoleModel) {
  if (!entry.runtimeIdle || !runtime) return
  const idle = entry.runtimeIdle
  void entry.model.motion(idle.group, idle.index, runtime.engine.MotionPriority.IDLE, {
    loop: idle.loop ?? true,
    resetExpression: false,
  })
}

function freezeModelFocus(entry: RoleModel) {
  const focusController = entry.model.internalModel.focusController
  focusController.focus(focusController.x, focusController.y, true)
  entry.focusFrozen = true
}

function finishReaction(entry: RoleModel, sequence: number) {
  if (sequence !== entry.reactionSequence) return
  entry.reactionLifecycleCleanup?.()
  entry.reactionLifecycleCleanup = null
  entry.focusFrozen = false
  updateModelFocus(entry)
}

function applyEmotion(entry: RoleModel, emotion: string) {
  if (entry.emotion === emotion || !runtime) return
  entry.emotion = emotion
  const expression = entry.variant.expressions[emotion] ?? entry.variant.default_expression
  if (expression) {
    void entry.model
      .expression(expression)
      .catch((error: unknown) =>
        console.warn(`[Live2D] expression failed for role ${entry.roleId}`, error),
      )
  }
  const motion = entry.variant.motions[emotion]
  if (motion) {
    const sequence = ++entry.reactionSequence
    freezeModelFocus(entry)
    const motionManager = entry.model.internalModel.motionManager
    entry.reactionLifecycleCleanup?.()
    entry.reactionLifecycleCleanup = trackMotionLifecycle(
      motionManager,
      motion.group,
      motion.index,
      runtime.engine.MotionPriority.FORCE,
      () => finishReaction(entry, sequence),
    )
    void entry.model
      .motion(motion.group, motion.index, runtime.engine.MotionPriority.FORCE, {
        loop: motion.loop ?? false,
        resetExpression: false,
      })
      .then((started: boolean) => {
        if (!started) finishReaction(entry, sequence)
      })
      .catch((error: unknown) => {
        finishReaction(entry, sequence)
        console.warn(`[Live2D] motion failed for role ${entry.roleId}`, error)
      })
  }
}

function destroyEntry(entry: RoleModel) {
  entry.reactionSequence += 1
  entry.reactionLifecycleCleanup?.()
  entry.reactionLifecycleCleanup = null
  application?.stage.removeChild(entry.model)
  destroyModel(entry.model)
  models.delete(entry.roleId)
  emitActiveRoles()
}

async function loadRole(
  role: GameRole,
  variantName: string,
  variant: Live2dVariant,
  requestId: number,
) {
  await ensureApplication()
  if (!application || !runtime || disposed) return
  let pendingModel: any = null
  const previous = models.get(role.roleId)
  let previousDetached = false
  try {
    const source = await loadModelSource(role.roleId, variant.model)
    const runtimeIdle = configureRuntimeIdle(source, variant.idle)
    const model = await runtime.engine.Live2DModel.from(source, {
      ticker: application.ticker,
      anchorMode: 'drawable',
      autoFocus: false,
      autoHitTest: false,
      eyeBlink: true,
      idleMotionGroup: runtimeIdle?.group ?? variant.idle?.group ?? 'Idle',
      motionPreload: runtime.engine.MotionPreloadStrategy.IDLE,
      useHighPrecisionMask: 'auto',
      textureOptions: { lod: 'single-auto' },
    })
    pendingModel = model
    const currentRole = props.roles.find((item) => item.roleId === role.roleId)
    if (
      disposed ||
      requestId !== requestSequenceFor(role.roleId) ||
      !currentRole ||
      variantNameFor(currentRole) !== variantName
    ) {
      destroyModel(model)
      pendingModel = null
      return
    }
    const entry: RoleModel = {
      roleId: role.roleId,
      variantName,
      model,
      variant,
      runtimeIdle,
      emotion: '',
      requestId,
      mouthParameterIndex: -1,
      mouthValue: 0,
      eyeLeftParameterIndex: -1,
      eyeRightParameterIndex: -1,
      eyesOpen: true,
      focusFrozen: false,
      reactionSequence: 0,
      reactionLifecycleCleanup: null,
    }
    if (variant.lip_sync?.parameter) {
      entry.mouthParameterIndex = findParameterIndex(entry, variant.lip_sync.parameter)
    }
    if (variant.eye_blink) {
      entry.eyeLeftParameterIndex = findParameterIndex(entry, variant.eye_blink.left)
      entry.eyeRightParameterIndex = findParameterIndex(entry, variant.eye_blink.right)
    }
    model.internalModel.on('beforeModelUpdate', () => {
      const coreModel = model.internalModel.coreModel as {
        addParameterValueByIndex(index: number, value: number, weight?: number): void
        getParameterValueByIndex(index: number): number
      }
      if (entry.mouthParameterIndex >= 0) {
        coreModel.addParameterValueByIndex(entry.mouthParameterIndex, entry.mouthValue, 1)
      }
      const eyeValues: number[] = []
      if (entry.eyeLeftParameterIndex >= 0) {
        eyeValues.push(coreModel.getParameterValueByIndex(entry.eyeLeftParameterIndex))
      }
      if (entry.eyeRightParameterIndex >= 0) {
        eyeValues.push(coreModel.getParameterValueByIndex(entry.eyeRightParameterIndex))
      }
      entry.eyesOpen = areEyesOpen(eyeValues)
      updateModelFocus(entry)
    })
    if (previous) {
      application.stage.removeChild(previous.model)
      previousDetached = true
    }
    application.stage.addChild(model)
    applyLayout(entry, role)
    // Verify the new model in isolation before replacing the active variant.
    application.render()
    if (previous) destroyEntry(previous)
    models.set(role.roleId, entry)
    pendingModel = null
    startIdle(entry)
    applyEmotion(entry, mappedEmotion(role.emotion))
    failedRoleIds.delete(role.roleId)
    emitFailedRoles()
    emitActiveRoles()
  } catch (error) {
    if (pendingModel) {
      application?.stage.removeChild(pendingModel)
      destroyModel(pendingModel)
    }
    if (requestId === requestSequenceFor(role.roleId)) {
      const current = models.get(role.roleId)
      if (current) {
        if (previousDetached && !application.stage.children.includes(current.model)) {
          application.stage.addChild(current.model)
          applyLayout(current, role)
        }
        failedRoleIds.delete(role.roleId)
      } else {
        failedRoleIds.add(role.roleId)
      }
      emitFailedRoles()
    }
    console.warn(`[Live2D] model load failed for role ${role.roleId}; keeping static avatar`, error)
  }
}

const roleRequests = new Map<number, number>()
function nextRequest(roleId: number) {
  const id = ++requestSequence
  roleRequests.set(roleId, id)
  return id
}
function requestSequenceFor(roleId: number) {
  return roleRequests.get(roleId)
}

async function syncRoles() {
  const liveRoles = props.roles.filter((role) => role.live2d)
  const liveIds = new Set(liveRoles.map((role) => role.roleId))
  let failedChanged = false
  for (const roleId of [...failedRoleIds]) {
    if (!liveIds.has(roleId)) {
      failedRoleIds.delete(roleId)
      failedChanged = true
    }
  }
  if (failedChanged) emitFailedRoles()
  for (const entry of [...models.values()]) {
    if (!liveIds.has(entry.roleId)) {
      nextRequest(entry.roleId)
      failedRoleIds.delete(entry.roleId)
      emitFailedRoles()
      destroyEntry(entry)
    }
  }
  if (!liveRoles.length) {
    destroyApplication()
    return
  }
  await ensureApplication()
  for (const [index, role] of liveRoles.entries()) {
    const settings = role.live2d
    if (!settings) continue
    const variantName = variantNameFor(role)
    const variant = variantName ? resolveLive2dVariant(settings, role.clothesName) : undefined
    if (!variantName || !variant) continue
    const entry = models.get(role.roleId)
    if (
      !entry ||
      entry.variantName !== variantName ||
      entry.variant.model !== variant.model ||
      !motionBindingEquals(entry.variant.idle, variant.idle)
    ) {
      await loadRole(role, variantName, variant, nextRequest(role.roleId))
      continue
    }
    if (entry.variant !== variant) {
      entry.variant = variant
      entry.emotion = ''
      startIdle(entry)
    }
    applyLayout(entry, role)
    applyEmotion(entry, mappedEmotion(role.emotion))
    application.stage.setChildIndex(
      entry.model,
      Math.min(index, application.stage.children.length - 1),
    )
  }
}

function queueSync() {
  syncPromise = syncPromise
    .then(syncRoles)
    .catch((error) => console.warn('[Live2D] stage sync failed', error))
}

function updateLipSync() {
  const audio = props.audioElement
  for (const entry of models.values()) {
    const isSpeaker =
      entry.roleId === props.activeSpeakerId && audio && !audio.paused && !audio.ended
    const target = isSpeaker
      ? sampleVoiceAmplitude(decodedVoice, audio.currentTime) * (entry.variant.lip_sync?.gain ?? 1)
      : 0
    entry.mouthValue += (Math.min(1, target) - entry.mouthValue) * 0.38
  }
}

watch(
  () =>
    props.roles.map(
      (role) =>
        [
          role.roleId,
          role.emotion,
          role.clothesName,
          role.show,
          role.scale,
          role.offsetX,
          role.offsetY,
          role.scaleP,
          role.offsetXP,
          role.offsetYP,
          role.live2d,
        ] as const,
    ),
  queueSync,
  { deep: true },
)

watch(
  () => [props.voiceDataUrl, props.roles.some((role) => Boolean(role.live2d))] as const,
  async ([url, hasLive2dRole]) => {
    const id = ++decodeSequence
    decodedVoice = null
    if (!hasLive2dRole) return
    const decoded = await decodeVoiceForLipSync(url)
    if (id === decodeSequence) decodedVoice = decoded
  },
)

onMounted(() => {
  // 桌宠模式：窗口非全屏，DOM pointermove 在鼠标移出窗口后停发，视线会冻结在
  // 最后一次窗口内位置。除窗口内 DOM 监听外，还需订阅 Rust 侧全局鼠标轮询
  // （每 50ms 上报窗口内逻辑坐标，即 webview 视口坐标，与 clientX/clientY 同源）。
  if (props.mode === 'pet') {
    window.addEventListener('pointermove', handlePointerMove, { passive: true })
    void listen<CursorPayload>('pet:cursor', (event) => {
      pointerPosition = { clientX: event.payload.x, clientY: event.payload.y }
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten()
          return
        }
        cursorUnlisten = unlisten
      })
      .catch(() => {
        // 非 Tauri 环境或事件系统不可用时静默降级（DOM 监听仍覆盖窗口内移动）
      })
  }
  queueSync()
})
onBeforeUnmount(() => {
  disposed = true
  if (props.mode === 'pet') {
    window.removeEventListener('pointermove', handlePointerMove)
    cursorUnlisten?.()
    cursorUnlisten = null
  }
  decodeSequence += 1
  for (const entry of [...models.values()]) destroyEntry(entry)
  destroyApplication()
})
</script>
