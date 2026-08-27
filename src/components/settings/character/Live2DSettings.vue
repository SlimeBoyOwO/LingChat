<template>
  <div class="space-y-5">
    <div class="flex
      flex-wrap
      gap-2">
      <button
        v-if="!isAndroid()"
        type="button"
        class="rounded-lg
          bg-white/10
          px-3
          py-2
          text-sm
          hover:bg-white/20"
        @click="pickSource('directory')"
      >
        {{ t('settings.characterInfo.live2d.importDirectory') }}
      </button>
      <button
        type="button"
        class="rounded-lg
          bg-white/10
          px-3
          py-2
          text-sm
          hover:bg-white/20"
        @click="pickSource('zip')"
      >
        {{ t('settings.characterInfo.live2d.importZip') }}
      </button>
      <button
        v-if="localSettings"
        type="button"
        class="rounded-lg
          bg-red-500/15
          px-3
          py-2
          text-sm
          text-red-200
          hover:bg-red-500/30"
        @click="removeLive2d"
      >
        {{ t('settings.characterInfo.live2d.remove') }}
      </button>
    </div>

    <p
      v-if="busy"
      class="text-sm
        text-cyan-200"
    >
      {{ t('settings.characterInfo.live2d.loading') }}
    </p>
    <p
      v-if="errorMessage"
      class="text-sm
        text-red-300"
    >
      {{ errorMessage }}
    </p>
    <p
      v-if="!localSettings && !busy"
      class="py-8
        text-center
        text-sm
        text-white/45"
    >
      {{ t('settings.characterInfo.live2d.empty') }}
    </p>

    <template v-if="localSettings">
      <div class="grid
        grid-cols-1
        gap-4
        md:grid-cols-[minmax(0,1fr)_240px]">
        <div class="space-y-4">
          <label class="flex
            flex-col
            gap-2
            text-sm
            text-white/70">
            {{ t('settings.characterInfo.live2d.defaultVariant') }}
            <select
              :value="localSettings.default_variant"
              class="live2d-control"
              @change="setDefaultVariant(($event.target as HTMLSelectElement).value)"
            >
              <option
                v-for="name in variantNames"
                :key="name"
                :value="name"
              >
                {{ name }}
              </option>
            </select>
          </label>

          <label class="flex
            flex-col
            gap-2
            text-sm
            text-white/70">
            {{ t('settings.characterInfo.live2d.editVariant') }}
            <select
              v-model="selectedVariant"
              class="live2d-control"
            >
              <option
                v-for="name in variantNames"
                :key="name"
                :value="name"
              >
                {{ name }}
              </option>
            </select>
          </label>

          <div
            v-if="currentVariant"
            class="space-y-3
              rounded-lg
              border
              border-white/10
              bg-white/5
              p-3"
          >
            <div class="break-all
              text-xs
              text-white/55">{{ currentVariant.model }}</div>
            <label class="flex
              flex-col
              gap-2
              text-sm
              text-white/70">
              {{ t('settings.characterInfo.live2d.defaultExpression') }}
              <select
                v-model="currentVariant.default_expression"
                class="live2d-control"
              >
                <option value="">-</option>
                <option
                  v-for="name in expressionOptions"
                  :key="name"
                  :value="name"
                >
                  {{ name }}
                </option>
              </select>
              <span class="text-xs text-white/45">
                {{ t('settings.characterInfo.live2d.defaultExpressionHint') }}
              </span>
            </label>
            <div class="grid
              grid-cols-2
              gap-2">
              <label class="flex
                flex-col
                gap-2
                text-sm
                text-white/70">
                {{ t('settings.characterInfo.live2d.focusAnchorX') }}
                <input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  class="live2d-control"
                  :value="currentVariant.focus_anchor?.x ?? 0.5"
                  @input="setFocusAnchor('x', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="flex
                flex-col
                gap-2
                text-sm
                text-white/70">
                {{ t('settings.characterInfo.live2d.focusAnchorY') }}
                <input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  class="live2d-control"
                  :value="currentVariant.focus_anchor?.y ?? 0.5"
                  @input="setFocusAnchor('y', ($event.target as HTMLInputElement).value)"
                />
              </label>
            </div>
            <button
              v-if="currentVariant.focus_anchor"
              type="button"
              class="text-left
                text-xs
                text-white/50
                hover:text-white/80"
              @click="currentVariant.focus_anchor = null"
            >
              {{ t('settings.characterInfo.live2d.focusAnchorReset') }}
            </button>
            <div class="grid
              grid-cols-1
              gap-2
              md:grid-cols-2">
              <div
                v-for="emotion in emotions"
                :key="emotion"
                class="rounded-lg
                  bg-black/15
                  p-2"
              >
                <div class="mb-2
                  text-xs
                  font-medium
                  text-white/70">{{ emotion }}</div>
                <select
                  v-model="currentVariant.expressions[emotion]"
                  class="live2d-control
                    mb-2"
                >
                  <option value="">{{ t('settings.characterInfo.live2d.noExpression') }}</option>
                  <option
                    v-for="name in expressionOptions"
                    :key="name"
                    :value="name"
                  >
                    {{ name }}
                  </option>
                </select>
                <select
                  class="live2d-control"
                  :value="motionValue(emotion)"
                  @change="setMotion(emotion, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t('settings.characterInfo.live2d.noMotion') }}</option>
                  <option
                    v-for="motion in motionOptions"
                    :key="motion.value"
                    :value="motion.value"
                  >
                    {{ motion.label }}
                  </option>
                </select>
              </div>
            </div>
          </div>

          <div
            v-if="clothesNames.length"
            class="space-y-2
              rounded-lg
              border
              border-white/10
              bg-white/5
              p-3"
          >
            <h4 class="text-sm
              font-semibold
              text-white/75">
              {{ t('settings.characterInfo.live2d.clothesMapping') }}
            </h4>
            <label
              v-for="clothes in clothesNames"
              :key="clothes"
              class="grid
                grid-cols-2
                items-center
                gap-3
                text-sm"
            >
              <span>{{ clothes }}</span>
              <select
                v-model="localSettings.clothes_variants[clothes]"
                class="live2d-control"
              >
                <option value="">{{ localSettings.default_variant }}</option>
                <option
                  v-for="name in variantNames"
                  :key="name"
                  :value="name"
                >
                  {{ name }}
                </option>
              </select>
            </label>
          </div>
        </div>

        <div class="h-60
          overflow-hidden
          rounded-lg
          border
          border-white/10
          bg-black/25">
          <Live2DStage
            v-if="previewRole"
            class="relative!
              h-full
              w-full"
            :roles="[previewRole]"
            mode="standard"
            :active-speaker-id="null"
            :audio-element="null"
            voice-data-url=""
          />
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { importLive2d, inspectLive2d } from '@/api/services/character'
import Live2DStage from '@/components/game/live2d/Live2DStage.vue'
import type { GameRole } from '@/stores/modules/game/state'
import type { Live2dImportResult, Live2dSettings } from '@/types/live2d'
import { isAndroid } from '@/utils/platform'

const props = defineProps<{
  roleId: number
  characterFolder: string
  clothes: Array<{ name?: string }> | null | undefined
  scale?: number
  offsetX?: number
  offsetY?: number
  modelValue: Live2dSettings | null | undefined
}>()

const emit = defineEmits<{
  'update:modelValue': [value: Live2dSettings | null]
}>()

const { t } = useI18n()
const localSettings = computed<Live2dSettings | null>({
  get: () => props.modelValue ?? null,
  set: (value) => emit('update:modelValue', value),
})
const metadata = ref<Live2dImportResult['models']>([])
const selectedVariant = ref('')
const busy = ref(false)
const errorMessage = ref('')

const emotions = [
  '正常',
  '平静',
  '高兴',
  '兴奋',
  '生气',
  '害羞',
  '疑惑',
  '哭泣',
  '惊讶',
  '厌恶',
  '担心',
  '认真',
  '紧张',
  '害怕',
  '慌张',
  '无奈',
  '心动',
  '调皮',
  '难为情',
  '自信',
]

const variantNames = computed(() => Object.keys(localSettings.value?.variants ?? {}))
const currentVariant = computed(() => localSettings.value?.variants[selectedVariant.value])
const currentMetadata = computed(() =>
  metadata.value.find((item) => item.variant === selectedVariant.value),
)
const expressionOptions = computed(() => currentMetadata.value?.expressions ?? [])
const motionOptions = computed(() => {
  const options: Array<{ value: string; label: string }> = []
  for (const [group, files] of Object.entries(currentMetadata.value?.motions ?? {})) {
    files.forEach((file, index) =>
      options.push({ value: `${group}:${index}`, label: `${group}[${index}] ${file}` }),
    )
  }
  return options
})
const clothesNames = computed(() => [
  'default',
  ...(props.clothes ?? [])
    .map((item) => item.name?.trim())
    .filter((name): name is string => Boolean(name)),
])

const previewRole = computed<GameRole | null>(() => {
  if (!localSettings.value || !selectedVariant.value) return null
  const previewSettings: Live2dSettings = {
    ...localSettings.value,
    default_variant: selectedVariant.value,
    clothes_variants: { default: selectedVariant.value },
  }
  return {
    roleId: props.roleId,
    roleName: '',
    roleSubTitle: '',
    thinkMessage: '',
    emotion: '正常',
    originalEmotion: '正常',
    scale: props.scale ?? 1,
    offsetY: props.offsetY ?? 0,
    offsetX: props.offsetX ?? 0,
    scaleP: 1,
    offsetXP: 0,
    offsetYP: 0,
    bubbleTop: 0,
    bubbleLeft: 0,
    show: true,
    clothes: {},
    clothesName: 'default',
    bodyPart: {},
    live2d: previewSettings,
    character_folder: props.characterFolder,
  }
})

watch(
  () => props.modelValue?.default_variant,
  (defaultVariant) => {
    if (!selectedVariant.value || !props.modelValue?.variants[selectedVariant.value]) {
      selectedVariant.value = defaultVariant ?? ''
    }
  },
  { immediate: true },
)

watch(
  () => props.roleId,
  async () => {
    if (!props.modelValue) return
    try {
      metadata.value = (await inspectLive2d(props.roleId)).models
    } catch (error) {
      console.warn('[Live2D] Failed to inspect current role models', error)
    }
  },
  { immediate: true },
)

function setFocusAnchor(axis: 'x' | 'y', rawValue: string) {
  const variant = currentVariant.value
  const value = Number(rawValue)
  if (!variant || !Number.isFinite(value)) return
  const anchor = variant.focus_anchor ?? { x: 0.5, y: 0.5 }
  variant.focus_anchor = { ...anchor, [axis]: Math.min(1, Math.max(0, value)) }
}

function setDefaultVariant(variantName: string) {
  const settings = localSettings.value
  if (!settings?.variants[variantName]) return
  settings.default_variant = variantName
  if (Object.prototype.hasOwnProperty.call(settings.clothes_variants, 'default')) {
    settings.clothes_variants.default = variantName
  }
  selectedVariant.value = variantName
}

function motionValue(emotion: string) {
  const motion = currentVariant.value?.motions[emotion]
  return motion ? `${motion.group}:${motion.index}` : ''
}

function setMotion(emotion: string, value: string) {
  const variant = currentVariant.value
  if (!variant) return
  if (!value) {
    delete variant.motions[emotion]
    return
  }
  const separator = value.lastIndexOf(':')
  variant.motions[emotion] = {
    ...variant.motions[emotion],
    group: value.slice(0, separator),
    index: Number(value.slice(separator + 1)),
    loop: false,
  }
}

async function pickSource(sourceKind: 'directory' | 'zip') {
  const selection = await open({
    directory: sourceKind === 'directory',
    multiple: false,
    filters: sourceKind === 'zip' ? [{ name: 'Live2D ZIP', extensions: ['zip'] }] : undefined,
  })
  if (typeof selection !== 'string') return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = await importLive2d(props.roleId, selection, sourceKind)
    metadata.value = result.models
    localSettings.value = result.live2d
    selectedVariant.value = result.live2d.default_variant
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    busy.value = false
  }
}

function removeLive2d() {
  localSettings.value = null
  metadata.value = []
  selectedVariant.value = ''
}
</script>

<style scoped>
.live2d-control {
  width: 100%;
  border: 1px solid rgb(255 255 255 / 0.1);
  border-radius: 0.5rem;
  background: rgb(0 0 0 / 0.2);
  padding: 0.45rem 0.65rem;
  color: white;
  font-size: 0.8rem;
}
.live2d-control option {
  background: #292929;
}
</style>
