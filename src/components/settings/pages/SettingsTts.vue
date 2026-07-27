<template>
  <MenuPage>
    <MenuItem title="本地 TTS" size="large">
      <template #header>
        <AudioLines :size="20" class="text-cyan-300" />
      </template>

      <div class="flex min-h-0 flex-col gap-6">
        <div class="flex flex-wrap items-center gap-x-6 gap-y-3 border-b border-white/10 pb-5">
          <label class="flex items-center gap-3">
            <input
              v-model="localTtsEnabled"
              type="checkbox"
              class="sr-only peer"
              :disabled="savingLocalTts"
              @change="saveLocalTtsSwitch"
            />
            <span
              class="relative h-5 w-9 rounded-full transition-colors"
              :class="localTtsEnabled ? 'bg-cyan-400/70' : 'bg-white/20'"
            >
              <span
                class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
                :class="localTtsEnabled ? 'translate-x-4' : 'translate-x-0'"
              ></span>
            </span>
            <span>
              <span class="block text-xs text-white/45">全局本地 TTS</span>
              <span class="block text-sm font-medium text-white">
                {{ localTtsEnabled ? '已启用' : '已关闭，使用云端 TTS' }}
              </span>
            </span>
          </label>
          <div class="h-8 w-px bg-white/10"></div>
          <div class="flex items-center gap-2">
            <span class="status-dot" :class="status?.ready ? 'ready' : 'blocked'"></span>
            <div>
              <p class="text-xs text-white/45">本地引擎</p>
              <p class="text-sm font-medium text-white">
                {{ status?.ready ? '已就绪' : '未就绪' }}
              </p>
            </div>
          </div>
          <div class="h-8 w-px bg-white/10"></div>
          <div>
            <p class="text-xs text-white/45">DeBERTa 与分词器</p>
            <p class="text-sm font-medium" :class="status?.deberta_installed ? 'text-emerald-300' : 'text-red-300'">
              {{ status?.deberta_installed ? '已安装' : '缺失' }}
            </p>
          </div>
          <div class="h-8 w-px bg-white/10"></div>
          <div>
            <p class="text-xs text-white/45">人物语音</p>
            <p class="text-sm font-medium text-white">{{ snapshot.voices.length }} 个</p>
          </div>
          <button
            class="icon-button ml-auto"
            title="刷新状态"
            :disabled="loading"
            @click="refreshAll"
          >
            <RefreshCw :size="16" :class="{ 'animate-spin': loading }" />
          </button>
        </div>

        <div
          v-if="status && !status.deberta_installed"
          class="flex items-start gap-3 border-l-2 border-red-400 bg-red-500/8 px-4 py-3 text-sm text-red-100"
        >
          <CircleAlert :size="18" class="mt-0.5 shrink-0 text-red-300" />
          <span>缺少 DeBERTa 模型或分词器，人物语音不能载入，也不能试听。</span>
        </div>

        <p
          v-if="notice"
          class="border-l-2 px-3 py-2 text-sm"
          :class="notice.kind === 'error' ? 'border-red-400 text-red-300' : 'border-emerald-400 text-emerald-300'"
        >
          {{ notice.text }}
        </p>

        <section>
          <div class="section-heading">
            <div>
              <h3>模型下载</h3>
              <p>从 ModelScope 一键拉取本地 TTS 所需的全部资产</p>
            </div>
            <FileDown :size="18" class="text-white/40" />
          </div>

          <ul class="flex flex-col gap-3">
            <li
              v-for="asset in TtsLocal.CATALOG"
              :key="asset.id"
              class="flex flex-col gap-2 rounded-lg border border-white/10 bg-white/5 p-4"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="font-medium text-white">{{ asset.display_name }}</p>
                  <p class="text-xs text-white/40">
                    {{ asset.source }} · {{ formatBytes(asset.size_bytes) }} · {{ asset.language }}
                  </p>
                </div>
                <button
                  class="action-button shrink-0"
                  :disabled="downloadingId === asset.id || rowState(asset.id) === 'installed'"
                  @click="triggerDownload(asset.id)"
                >
                  <LoaderCircle v-if="downloadingId === asset.id" :size="16" class="animate-spin" />
                  <Check v-else-if="rowState(asset.id) === 'installed'" :size="16" />
                  <FileDown v-else :size="16" />
                  <span>{{ rowLabel(asset.id) }}</span>
                </button>
              </div>
              <div v-if="progressByAsset[asset.id] !== undefined" class="flex items-center gap-3">
                <progress
                  class="h-2 flex-1 overflow-hidden rounded bg-white/10"
                  :value="progressByAsset[asset.id]"
                  max="100"
                />
                <span class="w-12 text-right text-xs text-white/40">
                  {{ Math.round(progressByAsset[asset.id] ?? 0) }}%
                </span>
              </div>
              <p
                v-if="downloadError[asset.id]"
                class="border-l-2 border-red-400 px-3 py-1 text-xs text-red-300"
              >
                {{ downloadError[asset.id] }}
              </p>
            </li>
          </ul>
        </section>

        <section>
          <div class="section-heading">
            <div>
              <h3>本地导入</h3>
              <p>支持原始模型文件、ZIP 和 7z 压缩包</p>
            </div>
            <HardDriveDownload :size="18" class="text-white/40" />
          </div>

          <div class="grid grid-cols-1 gap-3 lg:grid-cols-3">
            <button
              class="action-button"
              :disabled="busyAction !== null"
              @click="pickSharedAsset('deberta')"
            >
              <FileUp :size="17" />
              <span>导入 DeBERTa</span>
            </button>
            <button
              class="action-button"
              :disabled="busyAction !== null"
              @click="pickSharedAsset('deberta-tokenizer')"
            >
              <FileJson :size="17" />
              <span>导入分词器</span>
            </button>
            <div class="flex min-w-0 gap-2">
              <input
                v-model="importVoiceId"
                class="field min-w-0 flex-1"
                maxlength="64"
                placeholder="语音 ID（可选）"
                aria-label="导入语音 ID"
              />
              <button
                class="action-button shrink-0"
                :disabled="busyAction !== null"
                @click="pickVoice"
              >
                <FileArchive :size="17" />
                <span>导入语音</span>
              </button>
            </div>
          </div>
        </section>

        <section v-if="voicesMissingStyleVectors.length > 0">
          <div class="section-heading">
            <div>
              <h3>补齐 style_vectors</h3>
              <p>.onnx 语音需要同名的 style_vectors.json 才能在本地 TTS 中启用</p>
            </div>
            <Wand2 :size="18" class="text-white/40" />
          </div>

          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <select
              v-model="styleVectorsTarget"
              class="field h-9 min-w-0 flex-1 sm:max-w-72"
              :disabled="busyAction !== null"
            >
              <option value="">选择需要补齐的语音</option>
              <option
                v-for="voice in voicesMissingStyleVectors"
                :key="voice.voice_id"
                :value="voice.voice_id"
              >
                {{ voice.display_name || voice.voice_id }} ({{ voice.voice_id }})
              </option>
            </select>
            <button
              class="action-button shrink-0"
              :disabled="busyAction !== null || !styleVectorsTarget"
              @click="pickStyleVectors"
            >
              <FileJson :size="17" />
              <span>导入 style_vectors</span>
            </button>
          </div>
        </section>

        <section>
          <div class="section-heading">
            <div>
              <h3>已安装语音</h3>
              <p>{{ snapshot.voices.length }} 个可用人物模型</p>
            </div>
            <ListMusic :size="18" class="text-white/40" />
          </div>

          <div v-if="snapshot.voices.length === 0" class="empty-state">暂无人物语音</div>
          <div v-else class="divide-y divide-white/8 border-y border-white/10">
            <div
              v-for="voice in snapshot.voices"
              :key="voice.voice_id"
              class="grid min-h-16 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 py-3"
            >
              <div class="min-w-0">
                <p class="truncate text-sm font-medium text-white">
                  {{ voice.display_name || voice.voice_id }}
                </p>
                <p class="mt-1 truncate text-xs text-white/45">
                  {{ voice.voice_id }} · {{ voice.kind.toUpperCase() }} · {{ formatBytes(voice.size_bytes) }}
                </p>
                <p class="mt-1 flex items-center gap-1.5 text-[11px]">
                  <span
                    v-if="voice.kind === 'sbv2'"
                    class="kind-badge"
                    title=".sbv2 已内置 style_vectors"
                  >style_vectors 已内置</span>
                  <span
                    v-else-if="voice.has_style_vectors"
                    class="kind-badge"
                    title="已找到同名的 style_vectors.json"
                  >style_vectors 已配</span>
                  <span
                    v-else
                    class="kind-badge kind-badge-warn"
                    title="缺少 style_vectors.json，需在下方补齐后才能在本地 TTS 中启用该语音"
                  >缺 style_vectors</span>
                </p>
              </div>
              <button
                class="icon-button danger"
                title="删除语音"
                :disabled="busyAction !== null"
                @click="removeVoice(voice)"
              >
                <Trash2 :size="16" />
              </button>
            </div>
          </div>
        </section>

        <section>
          <div class="section-heading">
            <div>
              <h3>试听</h3>
              <p>输入内容并选择已安装的人物语音</p>
            </div>
            <Volume2 :size="18" class="text-white/40" />
          </div>

          <div class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_17rem]">
            <textarea
              v-model="previewText"
              class="field min-h-28 resize-y"
              maxlength="500"
              placeholder="输入试听文本"
              :disabled="!status?.ready"
            ></textarea>

            <div class="flex flex-col gap-3">
              <label class="control-label">
                <span>语音模型</span>
                <select v-model="previewVoice" class="field h-9" :disabled="!status?.ready">
                  <option value="">请选择</option>
                  <option v-for="voice in snapshot.voices" :key="voice.voice_id" :value="voice.voice_id">
                    {{ voice.display_name || voice.voice_id }}
                  </option>
                </select>
              </label>
              <label class="control-label">
                <span>语速 {{ previewSpeed.toFixed(2) }}x</span>
                <input v-model.number="previewSpeed" type="range" min="0.5" max="2" step="0.05" />
              </label>
              <label class="control-label">
                <span>随机度 {{ previewSdp.toFixed(2) }}</span>
                <input v-model.number="previewSdp" type="range" min="0" max="1" step="0.05" />
              </label>
            </div>
          </div>

          <div class="mt-4 flex flex-wrap items-center gap-3">
            <button
              class="primary-button"
              :disabled="!canPreview || previewing"
              @click="runPreview"
            >
              <LoaderCircle v-if="previewing" :size="16" class="animate-spin" />
              <Play v-else :size="16" />
              {{ previewing ? '生成中' : '生成试听' }}
            </button>
            <audio ref="audioRef" controls class="h-9 min-w-0 flex-1" />
          </div>
        </section>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import type { DialogFilter } from '@tauri-apps/plugin-dialog'
import {
  AudioLines,
  Check,
  CircleAlert,
  FileArchive,
  FileDown,
  FileJson,
  FileUp,
  HardDriveDownload,
  ListMusic,
  LoaderCircle,
  Play,
  RefreshCw,
  Trash2,
  Volume2,
  Wand2,
} from 'lucide-vue-next'
import { MenuItem, MenuPage } from '@/components/ui'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import * as TtsLocal from '@/api/services/tts-local'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import { speedToLengthScale } from '@/utils/tts-speed'
import { catalogRowState } from '@/utils/tts-download-state'
import type {
  TtsLocalInstallSnapshot,
  TtsLocalStatus,
  VoiceRecord,
} from '@/api/services/tts-local'

const dialogStore = useDialogStore()
const status = ref<TtsLocalStatus | null>(null)
const snapshot = ref<TtsLocalInstallSnapshot>({ assets: [], voices: [] })
const loading = ref(false)
const busyAction = ref<string | null>(null)
const importVoiceId = ref('')
const styleVectorsTarget = ref('')
const notice = ref<{ kind: 'success' | 'error'; text: string } | null>(null)
const previewText = ref('こんにちは、これはローカル音声のテストです。')
const previewVoice = ref('')
const previewSpeed = ref(1)
const previewSdp = ref(0)
const previewing = ref(false)
const audioRef = ref<HTMLAudioElement | null>(null)
let audioUrl: string | null = null
const progressByAsset = ref<Record<string, number>>({})
const downloadError = ref<Record<string, string>>({})
const downloadingId = ref<string | null>(null)
const localTtsEnabled = ref(false)
const savingLocalTts = ref(false)
let unlistenProgress: (() => void) | null = null

type FilterIntent = 'deberta' | 'tokenizer' | 'voice' | 'style_vectors'

// Android plugin-dialog interprets the `extensions` field as MIME types
// (not file extensions). ONNX / SBV2 have no registered MIME, so they fall
// back to application/octet-stream; the backend validates the actual file
// via archive::inspect_package and rejects unknown formats.
function dialogFilters(intent: FilterIntent): DialogFilter[] {
  if (/android/i.test(navigator.userAgent)) {
    switch (intent) {
      case 'deberta':
        return [{ name: 'ONNX model', extensions: ['application/octet-stream'] }]
      case 'tokenizer':
        return [{ name: 'Tokenizer', extensions: ['application/json', 'text/json'] }]
      case 'voice':
        return [{
          name: 'Voice model',
          extensions: [
            'application/zip',
            'application/x-7z-compressed',
            'application/octet-stream',
          ],
        }]
      case 'style_vectors':
        return [{ name: 'style_vectors JSON', extensions: ['application/json', 'text/json'] }]
    }
  }
  switch (intent) {
    case 'deberta':
      return [{ name: 'ONNX model', extensions: ['onnx'] }]
    case 'tokenizer':
      return [{ name: 'Tokenizer', extensions: ['json'] }]
    case 'voice':
      return [{ name: 'SBV2 voice', extensions: ['sbv2', 'onnx', 'zip', '7z'] }]
    case 'style_vectors':
      return [{ name: 'style_vectors JSON', extensions: ['json'] }]
  }
}

const canPreview = computed(
  () => Boolean(status.value?.ready && previewVoice.value && previewText.value.trim()),
)

const voicesMissingStyleVectors = computed(
  () =>
    snapshot.value.voices.filter(
      (voice) => voice.kind === 'onnx' && !voice.has_style_vectors,
    ),
)

function errorText(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  return JSON.stringify(error)
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  return `${(bytes / 1024 ** index).toFixed(index >= 2 ? 1 : 0)} ${units[index]}`
}

function selectedPath(value: string | string[] | null): string | null {
  if (typeof value === 'string') return value
  return value?.[0] ?? null
}

function normalizeVoiceId(value: string): string {
  const fileName = value.split(/[\\/]/).pop()?.replace(/\.(sbv2|onnx|zip|7z)$/i, '') || 'local-voice'
  const normalized = fileName
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return (normalized || 'local-voice').slice(0, 64)
}

async function refreshAll(): Promise<void> {
  loading.value = true
  try {
    const [nextStatus, nextSnapshot] = await Promise.all([
      TtsLocal.status(),
      TtsLocal.listInstalled(),
    ])
    status.value = nextStatus
    snapshot.value = nextSnapshot
    if (!previewVoice.value || !nextSnapshot.voices.some((voice) => voice.voice_id === previewVoice.value)) {
      previewVoice.value = nextSnapshot.voices[0]?.voice_id ?? ''
    }
  } catch (error) {
    notice.value = { kind: 'error', text: `读取本地 TTS 状态失败：${errorText(error)}` }
  } finally {
    loading.value = false
  }
}

async function pickSharedAsset(assetId: 'deberta' | 'deberta-tokenizer'): Promise<void> {
  const selection = await open({
    multiple: false,
    filters: [
      ...(assetId === 'deberta'
        ? dialogFilters('deberta')
        : dialogFilters('tokenizer')),
    ],
  })
  const path = selectedPath(selection)
  if (!path) return

  busyAction.value = `import:${assetId}`
  notice.value = null
  try {
    await TtsLocal.importFromPath(path, { assetId })
    notice.value = { kind: 'success', text: assetId === 'deberta' ? 'DeBERTa 已导入' : '分词器已导入' }
    await refreshAll()
  } catch (error) {
    notice.value = { kind: 'error', text: `导入失败：${errorText(error)}` }
  } finally {
    busyAction.value = null
  }
}

async function pickVoice(): Promise<void> {
  const selection = await open({
    multiple: false,
    filters: dialogFilters('voice'),
  })
  const path = selectedPath(selection)
  if (!path) return

  busyAction.value = 'import:voice'
  notice.value = null
  try {
    const voiceId = normalizeVoiceId(importVoiceId.value.trim() || path)
    await TtsLocal.importFromPath(path, { voiceId })
    importVoiceId.value = ''
    notice.value = { kind: 'success', text: `语音 ${voiceId} 已导入` }
    await refreshAll()
  } catch (error) {
    notice.value = { kind: 'error', text: `导入失败：${errorText(error)}` }
  } finally {
    busyAction.value = null
  }
}

async function pickStyleVectors(): Promise<void> {
  if (!styleVectorsTarget.value) {
    notice.value = { kind: 'error', text: '请先选择需要补齐 style_vectors 的语音' }
    return
  }
  const selection = await open({
    multiple: false,
    filters: dialogFilters('style_vectors'),
  })
  const path = selectedPath(selection)
  if (!path) return

  const target = styleVectorsTarget.value
  busyAction.value = `style-vectors:${target}`
  notice.value = null
  try {
    await TtsLocal.importStyleVectors(target, path)
    notice.value = { kind: 'success', text: `${target} 的 style_vectors 已导入` }
    await refreshAll()
  } catch (error) {
    notice.value = { kind: 'error', text: `导入失败：${errorText(error)}` }
  } finally {
    busyAction.value = null
  }
}

async function removeVoice(voice: VoiceRecord): Promise<void> {
  const confirmed = await dialogStore.confirm(
    `确定删除语音“${voice.display_name || voice.voice_id}”吗？`,
    '删除本地语音',
  )
  if (!confirmed) return

  busyAction.value = `delete:${voice.voice_id}`
  notice.value = null
  try {
    await TtsLocal.deleteVoice(voice.voice_id)
    notice.value = { kind: 'success', text: '语音已删除' }
    await refreshAll()
  } catch (error) {
    notice.value = { kind: 'error', text: `删除失败：${errorText(error)}` }
  } finally {
    busyAction.value = null
  }
}

async function runPreview(): Promise<void> {
  if (!canPreview.value) return
  previewing.value = true
  notice.value = null
  try {
    const bytes = await TtsLocal.synthesizePreview({
      text: previewText.value.trim(),
      voiceId: previewVoice.value,
      lengthScale: speedToLengthScale(previewSpeed.value),
      sdpRatio: previewSdp.value,
    })
    if (audioUrl) URL.revokeObjectURL(audioUrl)
    audioUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'audio/wav' }))
    await nextTick()
    if (audioRef.value) {
      audioRef.value.src = audioUrl
      await audioRef.value.play()
    }
  } catch (error) {
    notice.value = { kind: 'error', text: `试听失败：${errorText(error)}` }
  } finally {
    previewing.value = false
  }
}

async function loadLocalTtsSwitch(): Promise<void> {
  try {
    const setting = await getEnvConfigByKey('features.enable_local_tts')
    localTtsEnabled.value = setting.value === 'true'
  } catch (error) {
    notice.value = { kind: 'error', text: `读取本地 TTS 开关失败：${errorText(error)}` }
  }
}

async function saveLocalTtsSwitch(): Promise<void> {
  savingLocalTts.value = true
  try {
    await saveEnvConfigSettings({
      'features.enable_local_tts': String(localTtsEnabled.value),
    })
    notice.value = {
      kind: 'success',
      text: localTtsEnabled.value
        ? '本地 TTS 已启用，使用本地 TTS 配置的角色将在下次调用时生效（或重启后生效）'
        : '本地 TTS 已关闭，使用本地 TTS 配置的角色将沿用云端 TTS',
    }
  } catch (error) {
    localTtsEnabled.value = !localTtsEnabled.value
    notice.value = { kind: 'error', text: `保存本地 TTS 开关失败：${errorText(error)}` }
  } finally {
    savingLocalTts.value = false
  }
}

function rowState(assetId: string) {
  const asset = TtsLocal.CATALOG.find((item) => item.id === assetId)
  if (!asset) return 'missing'
  return catalogRowState({
    asset,
    progressPercent: progressByAsset.value[assetId],
    errorMessage: downloadError.value[assetId],
    status: status.value,
    voices: snapshot.value.voices,
  })
}

function rowLabel(assetId: string): string {
  const state = rowState(assetId)
  if (state === 'installed') return '已安装'
  if (state === 'downloading') return '下载中'
  if (state === 'error') return '重试下载'
  return '下载'
}

watch(
  () => snapshot.value.voices,
  (voices) => {
    if (!voices.some((voice) => voice.voice_id === previewVoice.value)) {
      previewVoice.value = voices[0]?.voice_id ?? ''
    }
    if (
      styleVectorsTarget.value &&
      !voices.some((voice) => voice.voice_id === styleVectorsTarget.value)
    ) {
      styleVectorsTarget.value = ''
    }
  },
)

onMounted(async () => {
  await loadLocalTtsSwitch()
  await refreshAll()
  unlistenProgress = TtsLocal.onDownloadProgress((progress) => {
    progressByAsset.value = {
      ...progressByAsset.value,
      [progress.asset_id]: progress.percent,
    }
  })
})

onUnmounted(() => {
  if (audioUrl) URL.revokeObjectURL(audioUrl)
  unlistenProgress?.()
  unlistenProgress = null
})

async function triggerDownload(assetId: string): Promise<void> {
  if (downloadingId.value) return
  downloadingId.value = assetId
  const nextProgress = { ...progressByAsset.value }
  delete nextProgress[assetId]
  progressByAsset.value = nextProgress
  const nextErrors = { ...downloadError.value }
  delete nextErrors[assetId]
  downloadError.value = nextErrors
  try {
    await TtsLocal.download(assetId)
    await refreshAll()
    const completedProgress = { ...progressByAsset.value }
    completedProgress[assetId] = 100
    progressByAsset.value = completedProgress
  } catch (error) {
    downloadError.value = {
      ...downloadError.value,
      [assetId]: errorText(error),
    }
  } finally {
    downloadingId.value = null
  }
}
</script>

<style scoped>
.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.section-heading h3 {
  color: white;
  font-size: 14px;
  font-weight: 600;
}

.section-heading p {
  margin-top: 2px;
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
}

.status-dot {
  width: 9px;
  height: 9px;
  flex: none;
  border-radius: 50%;
}

.status-dot.ready {
  background: #6ee7b7;
  box-shadow: 0 0 8px rgba(110, 231, 183, 0.5);
}

.status-dot.blocked {
  background: #f87171;
  box-shadow: 0 0 8px rgba(248, 113, 113, 0.45);
}

.field {
  width: 100%;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.24);
  padding: 8px 10px;
  color: white;
  font-size: 13px;
  outline: none;
}

.field:focus {
  border-color: rgba(103, 232, 249, 0.65);
}

.field:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.action-button,
.primary-button,
.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.82);
  transition: background 0.18s ease, border-color 0.18s ease, color 0.18s ease;
}

.action-button {
  min-height: 36px;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.06);
  font-size: 13px;
}

.primary-button {
  min-height: 36px;
  padding: 8px 14px;
  border-color: rgba(103, 232, 249, 0.38);
  background: rgba(8, 145, 178, 0.35);
  color: #cffafe;
  font-size: 13px;
  font-weight: 600;
}

.icon-button {
  width: 34px;
  height: 34px;
  flex: none;
  background: rgba(255, 255, 255, 0.06);
}

.action-button:hover:not(:disabled),
.icon-button:hover:not(:disabled) {
  border-color: rgba(103, 232, 249, 0.4);
  background: rgba(103, 232, 249, 0.12);
  color: #cffafe;
}

.primary-button:hover:not(:disabled) {
  background: rgba(8, 145, 178, 0.52);
}

.icon-button.danger:hover:not(:disabled) {
  border-color: rgba(248, 113, 113, 0.45);
  background: rgba(248, 113, 113, 0.12);
  color: #fca5a5;
}

.action-button:disabled,
.primary-button:disabled,
.icon-button:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}

.kind-badge {
  flex: none;
  border: 1px solid rgba(103, 232, 249, 0.22);
  border-radius: 4px;
  background: rgba(8, 145, 178, 0.12);
  padding: 1px 5px;
  color: rgba(207, 250, 254, 0.75);
  font-size: 10px;
}

.empty-state {
.kind-badge-warn {
  border-color: rgba(248, 113, 113, 0.35);
  background: rgba(248, 113, 113, 0.1);
  color: #fecaca;
}
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  padding: 22px 0;
  text-align: center;
  color: rgba(255, 255, 255, 0.42);
  font-size: 13px;
}

.control-label {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: rgba(255, 255, 255, 0.62);
  font-size: 12px;
}

input[type='range'] {
  accent-color: #67e8f9;
}
</style>
