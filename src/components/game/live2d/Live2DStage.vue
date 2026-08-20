<template>
  <div
    ref="host"
    class="absolute
      inset-0
      pointer-events-none
      overflow-hidden"
    aria-hidden="true"
  ></div>
</template>

<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { getLive2dFilePath } from '@/api/services/character'
import { EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import type { GameRole } from '@/stores/modules/game/state'
import { resolveLive2dVariant, type Live2dVariant } from '@/types/live2d'
import { loadLive2dRuntime, type Live2dRuntime } from './live2d-runtime'
import { decodeVoiceForLipSync, sampleVoiceAmplitude, type DecodedVoice } from './useLive2dLipSync'

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

interface RoleModel {
  roleId: number
  variantName: string
  model: any
  variant: Live2dVariant
  emotion: string
  requestId: number
  mouthParameterIndex: number
  mouthValue: number
  eyeLeftParameterIndex: number
  eyeRightParameterIndex: number
  nextBlinkAt: number
  blinkStartedAt: number
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
const models = new Map<number, RoleModel>()
const failedRoleIds = new Set<number>()

function emitFailedRoles() {
  emit('failedChange', [...failedRoleIds])
}

function emitActiveRoles() {
  emit('activeChange', [...models.keys()])
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

const URL_SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/

function resolveModelReference(modelFile: string, reference: string): string {
  if (
    URL_SCHEME.test(reference) ||
    reference.startsWith('/') ||
    /^[a-zA-Z]:[\\/]/.test(reference)
  ) {
    throw new Error(`Live2D resource reference must be relative: ${reference}`)
  }
  const segments = modelFile.split('\\').join('/').split('/')
  segments.pop()
  for (const segment of reference.split('\\').join('/').split('/')) {
    if (!segment || segment === '.') continue
    if (segment === '..') {
      if (!segments.length)
        throw new Error(`Live2D resource escapes the role directory: ${reference}`)
      segments.pop()
    } else {
      segments.push(segment)
    }
  }
  return segments.join('/')
}

async function loadModelSource(roleId: number, modelFile: string) {
  const modelPath = await getLive2dFilePath(roleId, modelFile)
  const modelUrl = convertFileSrc(modelPath)
  const response = await fetch(modelUrl)
  if (!response.ok) throw new Error(`Failed to load Live2D settings: HTTP ${response.status}`)
  const source = (await response.json()) as Record<string, any>
  const references = source.FileReferences as Record<string, any> | undefined
  if (!references) throw new Error('Live2D model3 is missing FileReferences')

  const rewrite = async (reference: string) => {
    const relative = resolveModelReference(modelFile, reference)
    return convertFileSrc(await getLive2dFilePath(roleId, relative))
  }
  const rewrites: Promise<void>[] = []
  for (const key of ['Moc', 'Physics', 'Pose', 'UserData', 'DisplayInfo']) {
    if (typeof references[key] === 'string') {
      rewrites.push(
        rewrite(references[key]).then((url) => {
          references[key] = url
        }),
      )
    }
  }
  if (Array.isArray(references.Textures)) {
    references.Textures.forEach((reference: unknown, index: number) => {
      if (typeof reference === 'string') {
        rewrites.push(
          rewrite(reference).then((url) => {
            references.Textures[index] = url
          }),
        )
      }
    })
  }
  if (Array.isArray(references.Expressions)) {
    references.Expressions.forEach((expression: Record<string, unknown>) => {
      if (typeof expression.File === 'string') {
        rewrites.push(
          rewrite(expression.File).then((url) => {
            expression.File = url
          }),
        )
      }
    })
  }
  if (references.Motions && typeof references.Motions === 'object') {
    for (const motions of Object.values(references.Motions) as Array<
      Array<Record<string, unknown>>
    >) {
      for (const motion of motions) {
        if (typeof motion.File === 'string') {
          rewrites.push(
            rewrite(motion.File).then((url) => {
              motion.File = url
            }),
          )
        }
        if (typeof motion.Sound === 'string') {
          rewrites.push(
            rewrite(motion.Sound).then((url) => {
              motion.Sound = url
            }),
          )
        }
      }
    }
  }
  await Promise.all(rewrites)
  source.url = modelUrl
  return source
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

function applyLayout(entry: RoleModel, role: GameRole) {
  if (!application || !host.value) return
  const model = entry.model
  const width = model.internalModel.width || model.width || 1
  const height = model.internalModel.height || model.height || 1
  if (props.mode === 'pet') {
    const baseScale = Math.max(application.screen.width / width, application.screen.height / height)
    model.anchor.set(0.5, 0.5)
    model.scale.set(baseScale * (role.scaleP || 1))
    model.position.set(
      application.screen.width / 2 + (role.offsetXP || 0),
      application.screen.height / 2 + (role.offsetYP || 0),
    )
  } else {
    const index = props.roles.findIndex((item) => item.roleId === role.roleId)
    const count = props.roles.length
    const xPercent = index < 0 ? 0.5 : (index + 1) / (count + 1)
    const baseScale = application.screen.height / height
    model.anchor.set(0.5, 1)
    model.scale.set(baseScale * (role.scale || 1))
    model.position.set(
      application.screen.width * xPercent + (role.offsetX || 0),
      application.screen.height + (role.offsetY || 0),
    )
  }
  model.visible = role.show
  model.automator.autoUpdate = role.show
}

function startIdle(entry: RoleModel) {
  if (!entry.variant.idle || !runtime) return
  const idle = entry.variant.idle
  void entry.model.motion(idle.group, idle.index, runtime.engine.MotionPriority.IDLE, {
    loop: idle.loop ?? true,
    resetExpression: false,
  })
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
    void entry.model.motion(motion.group, motion.index, runtime.engine.MotionPriority.FORCE, {
      loop: motion.loop ?? false,
      resetExpression: false,
      onFinish: () => startIdle(entry),
    })
  }
}

function destroyEntry(entry: RoleModel) {
  application?.stage.removeChild(entry.model)
  entry.model.destroy({ children: true, texture: true, baseTexture: true })
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
  try {
    const source = await loadModelSource(role.roleId, variant.model)
    const model = await runtime.engine.Live2DModel.from(source, {
      ticker: application.ticker,
      anchorMode: 'canvas',
      autoFocus: false,
      autoHitTest: false,
      eyeBlink: false,
      idleMotionGroup: variant.idle?.group ?? 'Idle',
      motionPreload: runtime.engine.MotionPreloadStrategy.IDLE,
      useHighPrecisionMask: 'auto',
      textureOptions: { lod: 'single-auto' },
    })
    pendingModel = model
    if (
      disposed ||
      requestId !== requestSequenceFor(role.roleId) ||
      variantNameFor(props.roles.find((item) => item.roleId === role.roleId) ?? role) !==
        variantName
    ) {
      model.destroy({ children: true, texture: true, baseTexture: true })
      pendingModel = null
      return
    }
    const entry: RoleModel = {
      roleId: role.roleId,
      variantName,
      model,
      variant,
      emotion: '',
      requestId,
      mouthParameterIndex: -1,
      mouthValue: 0,
      eyeLeftParameterIndex: -1,
      eyeRightParameterIndex: -1,
      nextBlinkAt: performance.now() + 2500 + Math.random() * 3500,
      blinkStartedAt: 0,
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
        multiplyParameterValueByIndex(index: number, value: number, weight?: number): void
      }
      if (entry.mouthParameterIndex >= 0) {
        coreModel.addParameterValueByIndex(entry.mouthParameterIndex, entry.mouthValue, 1)
      }
      const now = performance.now()
      if (!entry.blinkStartedAt && now >= entry.nextBlinkAt) entry.blinkStartedAt = now
      if (entry.blinkStartedAt) {
        const progress = (now - entry.blinkStartedAt) / 180
        const openness = progress < 0.5 ? 1 - progress * 2 : Math.min(1, (progress - 0.5) * 2)
        if (entry.eyeLeftParameterIndex >= 0) {
          coreModel.multiplyParameterValueByIndex(entry.eyeLeftParameterIndex, openness, 1)
        }
        if (entry.eyeRightParameterIndex >= 0) {
          coreModel.multiplyParameterValueByIndex(entry.eyeRightParameterIndex, openness, 1)
        }
        if (progress >= 1) {
          entry.blinkStartedAt = 0
          entry.nextBlinkAt = now + 2500 + Math.random() * 3500
        }
      }
    })
    application.stage.addChild(model)
    applyLayout(entry, role)
    // Verify the render pipe before hiding the static fallback.
    application.render()
    const previous = models.get(role.roleId)
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
      pendingModel.destroy({ children: true, texture: true, baseTexture: true })
    }
    if (requestId === requestSequenceFor(role.roleId)) {
      failedRoleIds.add(role.roleId)
      emitFailedRoles()
      const current = models.get(role.roleId)
      if (current && current.variantName !== variantName) destroyEntry(current)
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
  if (!liveRoles.length) return
  await ensureApplication()
  for (const [index, role] of liveRoles.entries()) {
    const settings = role.live2d
    if (!settings) continue
    const variantName = variantNameFor(role)
    const variant = variantName ? resolveLive2dVariant(settings, role.clothesName) : undefined
    if (!variantName || !variant) continue
    const entry = models.get(role.roleId)
    if (!entry || entry.variantName !== variantName || entry.variant.model !== variant.model) {
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
  () => props.voiceDataUrl,
  async (url) => {
    const id = ++decodeSequence
    decodedVoice = null
    const decoded = await decodeVoiceForLipSync(url)
    if (id === decodeSequence) decodedVoice = decoded
  },
)

onMounted(queueSync)
onBeforeUnmount(() => {
  disposed = true
  decodeSequence += 1
  resizeObserver?.disconnect()
  resizeObserver = null
  for (const entry of [...models.values()]) destroyEntry(entry)
  if (application) {
    application.destroy({ removeView: true, releaseGlobalResources: false }, true)
    application = null
  }
})
</script>
