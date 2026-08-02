<template>
  <Transition name="modal">
    <div
      v-if="visible"
      class="fixed inset-0 z-60 flex items-center justify-center bg-slate-950/70 backdrop-blur-sm p-4"
      @click="handleClose"
    >
      <div
        class="w-full max-w-6xl h-[90vh] overflow-hidden rounded-3xl border border-white/20 bg-[radial-gradient(circle_at_10%_10%,rgba(251,191,36,0.12),transparent_35%),radial-gradient(circle_at_90%_20%,rgba(45,212,191,0.12),transparent_40%),linear-gradient(160deg,rgba(15,23,42,0.96),rgba(15,23,42,0.88))] text-white shadow-2xl"
        @click.stop
      >
        <div class="h-full flex flex-col">
          <div class="px-6 py-4 border-b border-white/10 flex items-center justify-between">
            <div>
              <h2 class="text-xl font-semibold tracking-wide">
                {{ $t('settings.characterCreate.header.title') }}
              </h2>
              <p class="text-sm text-white/60">{{ $t('settings.characterCreate.header.subtitle') }}</p>
            </div>
            <button
              class="h-9 w-9 rounded-full bg-white/10 hover:bg-white/20 transition"
              @click="handleClose"
            >
              ×
            </button>
          </div>

          <div class="px-6 pt-4">
            <div class="grid grid-cols-3 gap-2 rounded-xl bg-white/5 p-1">
              <button
                v-for="step in steps"
                :key="step.id"
                :class="[
                  'rounded-lg px-3 py-2 text-sm transition',
                  activeStep === step.id
                    ? 'bg-amber-300/20 text-amber-200'
                    : 'text-white/70 hover:bg-white/10',
                ]"
                @click="activeStep = step.id"
              >
                {{ step.label }}
              </button>
            </div>
          </div>

          <div class="flex-1 overflow-y-auto px-6 py-5 space-y-6">
            <section v-if="activeStep === 'basic'" class="space-y-4">
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.resourceFolder') }} *</label>
                  <input
                    v-model="form.resource_folder"
                    type="text"
                    :placeholder="$t('settings.characterCreate.form.resourceFolderPlaceholder')"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.title') }} *</label>
                  <input
                    v-model="form.title"
                    type="text"
                    :placeholder="$t('settings.characterCreate.form.titlePlaceholder')"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.aiName') }} *</label>
                  <input
                    v-model="form.ai_name"
                    type="text"
                    :placeholder="$t('settings.characterCreate.form.aiNamePlaceholder')"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.aiSubtitle') }}</label>
                  <input
                    v-model="form.ai_subtitle"
                    type="text"
                    :placeholder="$t('settings.characterCreate.form.aiSubtitlePlaceholder')"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.userName') }}</label>
                  <input
                    v-model="form.user_name"
                    type="text"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.userSubtitle') }}</label>
                  <input
                    v-model="form.user_subtitle"
                    type="text"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                  />
                </div>
              </div>
              <div class="space-y-2">
                <label class="text-sm text-white/70">{{ $t('settings.characterCreate.form.info') }}</label>
                <textarea
                  v-model="form.info"
                  rows="4"
                  :placeholder="$t('settings.characterCreate.form.infoPlaceholder')"
                  class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2 focus:outline-none focus:border-amber-300/70"
                ></textarea>
              </div>
            </section>

            <section v-else-if="activeStep === 'avatar'" class="space-y-5">
              <div
                class="rounded-xl border px-4 py-3"
                :class="
                  isAvatarComplete
                    ? 'border-emerald-400/40 bg-emerald-300/10'
                    : 'border-rose-400/40 bg-rose-300/10'
                "
              >
                <div class="text-sm font-medium">
                  {{ avatarStatusText }}
                </div>
                <div v-if="missingEmotionNames.length > 0" class="text-xs mt-1 text-rose-200/90">
                  {{ $t('settings.characterCreate.avatar.missing', { names: missingEmotionLabels }) }}
                </div>
              </div>

              <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
                <label
                  class="rounded-2xl border p-2 cursor-pointer transition flex flex-col gap-2"
                  :class="[
                    avatarFile
                      ? 'border-emerald-400/50 bg-emerald-300/10'
                      : 'border-rose-400/50 bg-white/5 hover:bg-white/10',
                    dragOver.avatar ? 'border-amber-400/70 bg-amber-300/20' : '',
                  ]"
                  @dragover.prevent="onDragOver('avatar', $event)"
                  @dragleave.prevent="onDragLeave('avatar')"
                  @drop.prevent="onAvatarDrop"
                >
                  <div class="text-xs text-white/80 flex justify-between">
                    <span>{{ $t('settings.characterCreate.avatar.avatarLabel') }} *</span>
                    <span>{{ avatarFile ? $t('settings.characterCreate.avatar.uploaded') : $t('settings.characterCreate.avatar.notUploaded') }}</span>
                  </div>
                  <div
                    class="aspect-square rounded-xl overflow-hidden bg-slate-900/60 border border-white/10"
                  >
                    <img
                      v-if="avatarPreviewUrl"
                      :src="avatarPreviewUrl"
                      alt="avatar preview"
                      class="h-full w-full object-cover"
                    />
                    <div
                      v-else
                      class="h-full w-full flex items-center justify-center text-xs text-white/40"
                    >
                      {{ $t('settings.characterCreate.avatar.dropHint') }}
                    </div>
                  </div>
                  <input type="file" accept="image/*" class="hidden" @change="onAvatarChange" />
                </label>

                <label
                  v-for="emotion in EMOTION_SLOTS"
                  :key="emotion"
                  class="rounded-2xl border p-2 cursor-pointer transition flex flex-col gap-2"
                  :class="[
                    emotionFiles[emotion]
                      ? 'border-emerald-400/50 bg-emerald-300/10'
                      : 'border-rose-400/50 bg-white/5 hover:bg-white/10',
                    dragOver.emotions[emotion] ? 'border-amber-400/70 bg-amber-300/20' : '',
                  ]"
                  @dragover.prevent="onEmotionDragOver(emotion, $event)"
                  @dragleave.prevent="onEmotionDragLeave(emotion)"
                  @drop.prevent="(event) => onEmotionDrop(emotion, event)"
                >
                  <div class="text-xs text-white/80 flex justify-between">
                    <span>{{ emotionLabel(emotion) }} *</span>
                    <span>{{ emotionFiles[emotion] ? $t('settings.characterCreate.avatar.uploaded') : $t('settings.characterCreate.avatar.notUploaded') }}</span>
                  </div>
                  <div
                    class="aspect-square rounded-xl overflow-hidden bg-slate-900/60 border border-white/10"
                  >
                    <img
                      v-if="emotionPreviewUrls[emotion]"
                      :src="emotionPreviewUrls[emotion]"
                      :alt="`${emotion} preview`"
                      class="h-full w-full object-cover"
                    />
                    <div
                      v-else
                      class="h-full w-full flex items-center justify-center text-xs text-white/40"
                    >
                      {{ $t('settings.characterCreate.avatar.dropHint') }}
                    </div>
                  </div>
                  <input
                    type="file"
                    accept="image/*"
                    class="hidden"
                    @change="(event) => onEmotionChange(emotion, event)"
                  />
                </label>
              </div>
            </section>

            <section v-else class="space-y-4">
              <button
                class="w-full md:w-auto rounded-xl px-4 py-2 bg-white/10 hover:bg-white/20 transition"
                @click="showAdvanced = !showAdvanced"
              >
                {{ showAdvanced ? $t('settings.characterCreate.advanced.collapse') : $t('settings.characterCreate.advanced.expand') }}
              </button>

              <div v-if="showAdvanced" class="space-y-4">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div class="space-y-2">
                    <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.scale') }}</label>
                    <input
                      v-model.number="form.scale"
                      type="number"
                      step="0.01"
                      class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                    />
                  </div>
                  <div class="space-y-2">
                    <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.offset') }}</label>
                    <input
                      v-model.number="form.offset"
                      type="number"
                      class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                    />
                  </div>
                  <div class="space-y-2">
                    <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.bubbleTop') }}</label>
                    <input
                      v-model.number="form.bubble_top"
                      type="number"
                      class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                    />
                  </div>
                  <div class="space-y-2">
                    <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.bubbleLeft') }}</label>
                    <input
                      v-model.number="form.bubble_left"
                      type="number"
                      class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                    />
                  </div>
                </div>

                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.thinkingMessage') }}</label>
                  <input
                    v-model="form.thinking_message"
                    type="text"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                  />
                </div>

                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.ttsType') }}</label>
                  <select
                    v-model="form.tts_type"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                  >
                    <option value="">{{ $t('settings.characterCreate.advanced.ttsNone') }}</option>
                    <option value="sva">sva</option>
                    <option value="sbv2">sbv2</option>
                    <option value="bv2">bv2</option>
                    <option value="sbv2api">sbv2api</option>
                    <option value="gsv">gsv</option>
                    <option value="aivis">aivis</option>
                    <option value="localsbv2api">localsbv2api</option>
                    <option value="indextts2">indextts2</option>
                  </select>
                </div>

                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.systemPrompt') }}</label>
                  <textarea
                    v-model="form.system_prompt"
                    rows="6"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                  ></textarea>
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.systemPromptExample') }}</label>
                  <textarea
                    v-model="form.system_prompt_example"
                    rows="5"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                  ></textarea>
                </div>
                <div class="space-y-2">
                  <label class="text-sm text-white/70">{{ $t('settings.characterCreate.advanced.systemPromptExampleOld') }}</label>
                  <textarea
                    v-model="form.system_prompt_example_old"
                    rows="4"
                    class="w-full rounded-xl bg-white/10 border border-white/20 px-3 py-2"
                  ></textarea>
                </div>
              </div>
            </section>
          </div>

          <div class="px-6 py-4 border-t border-white/10 flex items-center justify-between gap-3">
            <div class="text-sm text-rose-300 min-h-5">{{ errorMessage }}</div>
            <div class="flex items-center gap-2">
              <button
                class="rounded-xl px-4 py-2 bg-white/10 hover:bg-white/20 transition"
                @click="prevStep"
                :disabled="activeStep === 'basic'"
              >
                {{ $t('settings.characterCreate.footer.prevStep') }}
              </button>
              <button
                v-if="activeStep !== 'advanced'"
                class="rounded-xl px-4 py-2 bg-amber-400/80 text-slate-900 hover:bg-amber-300 transition disabled:opacity-50"
                @click="nextStep"
                :disabled="
                  (activeStep === 'basic' && !isBasicComplete) ||
                  (activeStep === 'avatar' && !isAvatarComplete)
                "
              >
                {{ $t('settings.characterCreate.footer.nextStep') }}
              </button>
              <button
                v-else
                class="rounded-xl px-4 py-2 bg-emerald-400/90 text-slate-900 hover:bg-emerald-300 transition disabled:opacity-50"
                :disabled="!canSubmit"
                @click="submitCreate"
              >
                {{ creating ? $t('settings.characterCreate.footer.creating') : $t('settings.characterCreate.footer.confirmCreate') }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { createCharacter } from '@/api/services/character'

type StepId = 'basic' | 'avatar' | 'advanced'

interface CharacterFormState {
  resource_folder: string
  title: string
  ai_name: string
  ai_subtitle: string
  user_name: string
  user_subtitle: string
  info: string
  scale: number
  offset: number
  bubble_top: number
  bubble_left: number
  thinking_message: string
  tts_type: string
  system_prompt: string
  system_prompt_example: string
  system_prompt_example_old: string
}

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  (event: 'close'): void
  (
    event: 'created',
    payload: { character_id: number; title: string; resource_folder: string },
  ): void
}>()

const EMOTION_SLOTS = [
  '兴奋',
  '厌恶',
  '哭泣',
  '害怕',
  '害羞',
  '平静',
  '心动',
  '惊讶',
  '慌张',
  '担心',
  '无奈',
  '生气',
  '疑惑',
  '紧张',
  '自信',
  '认真',
  '调皮',
  '难为情',
  '高兴',
  '正常',
] as const

const { t } = useI18n()

// 情绪槽位名称是后端协议字段，不能改动；仅通过该映射做界面显示的本地化
const EMOTION_KEY_MAP: Record<string, string> = {
  兴奋: 'excited',
  厌恶: 'disgusted',
  哭泣: 'crying',
  害怕: 'scared',
  害羞: 'shy',
  平静: 'calm',
  心动: 'heartFlutter',
  惊讶: 'surprised',
  慌张: 'flustered',
  担心: 'worried',
  无奈: 'helpless',
  生气: 'angry',
  疑惑: 'confused',
  紧张: 'nervous',
  自信: 'confident',
  认真: 'serious',
  调皮: 'playful',
  难为情: 'embarrassed',
  高兴: 'happy',
  正常: 'normal',
}

const emotionLabel = (emotion: string) => {
  const slug = EMOTION_KEY_MAP[emotion]
  return slug ? t(`settings.characterCreate.emotions.${slug}`) : emotion
}

const steps = computed<{ id: StepId; label: string }[]>(() => [
  { id: 'basic', label: t('settings.characterCreate.steps.basic') },
  { id: 'avatar', label: t('settings.characterCreate.steps.avatar') },
  { id: 'advanced', label: t('settings.characterCreate.steps.advanced') },
])

const activeStep = ref<StepId>('basic')
const showAdvanced = ref(false)
const creating = ref(false)
const errorMessage = ref('')

const form = reactive<CharacterFormState>({
  resource_folder: '',
  title: '',
  ai_name: '',
  ai_subtitle: '',
  user_name: '用户',
  user_subtitle: '',
  info: '',
  scale: 1,
  offset: 0,
  bubble_top: 5,
  bubble_left: 20,
  thinking_message: '正在思考中...',
  tts_type: '',
  system_prompt: '',
  system_prompt_example: '',
  system_prompt_example_old: '',
})

const avatarFile = ref<File | null>(null)
const avatarPreviewUrl = ref('')
const emotionFiles = reactive<Record<string, File | null>>({})
const emotionPreviewUrls = reactive<Record<string, string>>({})

// 拖拽状态
const dragOver = reactive({
  avatar: false,
  emotions: {} as Record<string, boolean>,
})

// 初始化情绪拖拽状态
for (const emotion of EMOTION_SLOTS) {
  dragOver.emotions[emotion] = false
}

const resetAll = () => {
  activeStep.value = 'basic'
  showAdvanced.value = false
  creating.value = false
  errorMessage.value = ''

  form.resource_folder = ''
  form.title = ''
  form.ai_name = ''
  form.ai_subtitle = ''
  form.user_name = '用户'
  form.user_subtitle = ''
  form.info = ''
  form.scale = 1
  form.offset = 0
  form.bubble_top = 5
  form.bubble_left = 20
  form.thinking_message = '正在思考中...'
  form.tts_type = ''
  form.system_prompt = ''
  form.system_prompt_example = ''
  form.system_prompt_example_old = ''

  if (avatarPreviewUrl.value) URL.revokeObjectURL(avatarPreviewUrl.value)
  avatarPreviewUrl.value = ''
  avatarFile.value = null

  for (const emotion of EMOTION_SLOTS) {
    const prev = emotionPreviewUrls[emotion]
    if (prev) URL.revokeObjectURL(prev)
    emotionFiles[emotion] = null
    emotionPreviewUrls[emotion] = ''
  }
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      resetAll()
    }
  },
)

const isBasicComplete = computed(() => {
  return (
    form.resource_folder.trim().length > 0 &&
    form.title.trim().length > 0 &&
    form.ai_name.trim().length > 0
  )
})

const missingEmotionNames = computed(() => {
  return EMOTION_SLOTS.filter((emotion) => !emotionFiles[emotion])
})

const uploadedEmotionCount = computed(() => {
  return EMOTION_SLOTS.filter((emotion) => emotionFiles[emotion]).length
})

const missingEmotionLabels = computed(() => {
  return missingEmotionNames.value.map(emotionLabel).join('、')
})

const avatarStatusText = computed(() => {
  const avatarText = avatarFile.value
    ? t('settings.characterCreate.avatar.avatarUploaded')
    : t('settings.characterCreate.avatar.avatarNotUploaded')
  return t('settings.characterCreate.avatar.uploadedStatus', {
    count: uploadedEmotionCount.value,
    avatar: avatarText,
  })
})

const isAvatarComplete = computed(() => {
  return Boolean(avatarFile.value) && missingEmotionNames.value.length === 0
})

const canSubmit = computed(() => {
  return isBasicComplete.value && isAvatarComplete.value && !creating.value
})

const setPreview = (target: 'avatar' | 'emotion', key: string, file: File) => {
  const newUrl = URL.createObjectURL(file)
  if (target === 'avatar') {
    if (avatarPreviewUrl.value) URL.revokeObjectURL(avatarPreviewUrl.value)
    avatarPreviewUrl.value = newUrl
    return
  }

  const prev = emotionPreviewUrls[key]
  if (prev) URL.revokeObjectURL(prev)
  emotionPreviewUrls[key] = newUrl
}

const onAvatarChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  avatarFile.value = file
  setPreview('avatar', 'avatar', file)
}

const onEmotionChange = (emotion: string, event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  emotionFiles[emotion] = file
  setPreview('emotion', emotion, file)
}

// 头像拖拽事件处理
const onDragOver = (type: 'avatar', event: DragEvent) => {
  if (type === 'avatar') {
    dragOver.avatar = true
  }
}

const onDragLeave = (type: 'avatar') => {
  if (type === 'avatar') {
    dragOver.avatar = false
  }
}

const onAvatarDrop = (event: DragEvent) => {
  dragOver.avatar = false
  const file = event.dataTransfer?.files?.[0]
  if (!file || !file.type.startsWith('image/')) return
  avatarFile.value = file
  setPreview('avatar', 'avatar', file)
}

// 情绪立绘拖拽事件处理
const onEmotionDragOver = (emotion: string, event: DragEvent) => {
  dragOver.emotions[emotion] = true
}

const onEmotionDragLeave = (emotion: string) => {
  dragOver.emotions[emotion] = false
}

const onEmotionDrop = (emotion: string, event: DragEvent) => {
  dragOver.emotions[emotion] = false
  const file = event.dataTransfer?.files?.[0]
  if (!file || !file.type.startsWith('image/')) return
  emotionFiles[emotion] = file
  setPreview('emotion', emotion, file)
}

const handleClose = () => {
  if (creating.value) return
  emit('close')
}

const prevStep = () => {
  if (activeStep.value === 'avatar') {
    activeStep.value = 'basic'
    return
  }
  if (activeStep.value === 'advanced') {
    activeStep.value = 'avatar'
  }
}

const nextStep = () => {
  errorMessage.value = ''
  if (activeStep.value === 'basic') {
    if (!isBasicComplete.value) {
      errorMessage.value = t('settings.characterCreate.errors.basicIncomplete')
      return
    }
    activeStep.value = 'avatar'
    return
  }
  if (activeStep.value === 'avatar') {
    if (!isAvatarComplete.value) {
      errorMessage.value = t('settings.characterCreate.errors.avatarIncomplete')
      return
    }
    activeStep.value = 'advanced'
  }
}

const submitCreate = async () => {
  if (!canSubmit.value || !avatarFile.value) return

  errorMessage.value = ''
  creating.value = true

  try {
    const settingsPayload = {
      ai_name: form.ai_name.trim(),
      ai_subtitle: form.ai_subtitle.trim(),
      user_name: form.user_name.trim() || '用户',
      user_subtitle: form.user_subtitle.trim(),
      title: form.title.trim(),
      info: form.info.trim(),
      scale: Number(form.scale),
      offset: Number(form.offset),
      bubble_top: Number(form.bubble_top),
      bubble_left: Number(form.bubble_left),
      thinking_message: form.thinking_message.trim() || '正在思考中...',
      tts_type: form.tts_type || null,
      system_prompt: form.system_prompt.trim() || null,
      system_prompt_example: form.system_prompt_example.trim() || null,
      system_prompt_example_old: form.system_prompt_example_old.trim() || null,
    }

    const formData = new FormData()
    formData.append('resource_folder', form.resource_folder.trim())
    formData.append('settings_json', JSON.stringify(settingsPayload))
    formData.append('avatar_file', avatarFile.value)

    for (const emotion of EMOTION_SLOTS) {
      const emotionFile = emotionFiles[emotion]
      if (!emotionFile) {
        throw new Error(
          t('settings.characterCreate.errors.missingEmotionFile', { name: emotionLabel(emotion) }),
        )
      }
      formData.append('emotion_names', emotion)
      formData.append('emotion_files', emotionFile)
    }

    const response = await createCharacter(formData)
    emit('created', response.data)
    emit('close')
  } catch (error: any) {
    errorMessage.value = error?.message || t('settings.characterCreate.errors.createFailed')
  } finally {
    creating.value = false
  }
}
</script>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: all 0.25s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
