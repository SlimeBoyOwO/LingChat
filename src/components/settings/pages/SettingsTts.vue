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
            <span
              class="h-[9px] w-[9px] shrink-0 rounded-full"
              :class="engineLoading
                ? 'bg-amber-300 shadow-[0_0_8px_rgba(252,211,77,0.5)]'
                : status?.ready
                  ? 'bg-emerald-300 shadow-[0_0_8px_rgba(110,231,183,0.5)]'
                  : 'bg-red-400 shadow-[0_0_8px_rgba(248,113,113,0.45)]'"
            ></span>
            <div>
              <p class="text-xs text-white/45">本地引擎</p>
              <p class="text-sm font-medium text-white">
                {{ engineLoading ? '加载中' : status?.ready ? '已就绪' : '未就绪' }}
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
            class="ml-auto inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
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
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">模型下载</h3>
              <p class="mt-0.5 text-xs text-white/45">从 ModelScope 一键拉取本地 TTS 所需的全部资产</p>
            </div>
            <FileDown :size="18" class="text-white/40" />
          </div>

          <ul class="flex flex-col gap-3">
            <li
              v-for="asset in catalog"
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
                  class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
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
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">本地导入</h3>
              <p class="mt-0.5 text-xs text-white/45">支持原始模型文件、ZIP 和 7z 压缩包</p>
            </div>
            <HardDriveDownload :size="18" class="text-white/40" />
          </div>

          <div class="grid grid-cols-1 gap-3 lg:grid-cols-3">
            <button
              class="inline-flex min-h-9 items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="busyAction !== null"
              @click="pickSharedAsset('deberta')"
            >
              <FileUp :size="17" />
              <span>导入 DeBERTa</span>
            </button>
            <button
              class="inline-flex min-h-9 items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="busyAction !== null"
              @click="pickSharedAsset('deberta-tokenizer')"
            >
              <FileJson :size="17" />
              <span>导入分词器</span>
            </button>
            <div class="flex min-w-0 gap-2">
              <input
                v-model="importVoiceId"
                class="w-full min-w-0 flex-1 rounded-md border border-white/15 bg-black/25 px-2.5 py-2 text-[13px] text-white outline-none transition-colors focus:border-cyan-300/65 disabled:cursor-not-allowed disabled:opacity-45"
                maxlength="64"
                placeholder="语音 ID（可选）"
                aria-label="导入语音 ID"
              />
              <button
                class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
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
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">补齐 style_vectors</h3>
              <p class="mt-0.5 text-xs text-white/45">.onnx 语音需要同名的 style_vectors.json 才能在本地 TTS 中启用</p>
            </div>
            <Wand2 :size="18" class="text-white/40" />
          </div>

          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <select
              v-model="styleVectorsTarget"
              class="h-9 w-full min-w-0 flex-1 rounded-md border border-white/15 bg-black/25 px-2.5 py-2 text-[13px] text-white outline-none transition-colors focus:border-cyan-300/65 disabled:cursor-not-allowed disabled:opacity-45 sm:max-w-72"
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
              class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="busyAction !== null || !styleVectorsTarget"
              @click="pickStyleVectors"
            >
              <FileJson :size="17" />
              <span>导入 style_vectors</span>
            </button>
          </div>
        </section>

        <section>
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">已安装语音</h3>
              <p class="mt-0.5 text-xs text-white/45">{{ snapshot.voices.length }} 个可用人物模型</p>
            </div>
            <ListMusic :size="18" class="text-white/40" />
          </div>

          <div v-if="snapshot.voices.length === 0" class="border-y border-white/10 py-[22px] text-center text-[13px] text-white/40">暂无人物语音</div>
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
                    class="shrink-0 rounded border border-cyan-300/25 bg-cyan-600/10 px-1 py-px text-[10px] text-cyan-50/75"
                    title=".sbv2 已内置 style_vectors"
                  >style_vectors 已内置</span>
                  <span
                    v-else-if="voice.has_style_vectors"
                    class="shrink-0 rounded border border-cyan-300/25 bg-cyan-600/10 px-1 py-px text-[10px] text-cyan-50/75"
                    title="已找到同名的 style_vectors.json"
                  >style_vectors 已配</span>
                  <span
                    v-else
                    class="shrink-0 rounded border border-cyan-300/25 bg-cyan-600/10 px-1 py-px text-[10px] text-cyan-50/75 border-red-400/35! bg-red-400/10! text-red-200!"
                    title="缺少 style_vectors.json，需在下方补齐后才能在本地 TTS 中启用该语音"
                  >缺 style_vectors</span>
                </p>
              </div>
              <button
                class="inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px] rounded-md border border-white/15 bg-white/5 text-white/80 transition-colors duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40 enabled:hover:border-red-400/50! enabled:hover:bg-red-400/10! enabled:hover:text-red-300!"
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
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">试听</h3>
              <p class="mt-0.5 text-xs text-white/45">输入内容并选择已安装的人物语音</p>
            </div>
            <Volume2 :size="18" class="text-white/40" />
          </div>

          <div class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_17rem]">
            <textarea
              v-model="previewText"
              class="min-h-28 w-full resize-y rounded-md border border-white/15 bg-black/25 px-2.5 py-2 text-[13px] text-white outline-none transition-colors focus:border-cyan-300/65 disabled:cursor-not-allowed disabled:opacity-45"
              maxlength="500"
              placeholder="输入试听文本"
              :disabled="!status?.ready"
            ></textarea>

            <div class="flex flex-col gap-3">
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>语音模型</span>
                <select v-model="previewVoice" class="h-9 w-full rounded-md border border-white/15 bg-black/25 px-2.5 py-2 text-[13px] text-white outline-none transition-colors focus:border-cyan-300/65 disabled:cursor-not-allowed disabled:opacity-45" :disabled="!status?.ready">
                  <option value="">请选择</option>
                  <option v-for="voice in snapshot.voices" :key="voice.voice_id" :value="voice.voice_id">
                    {{ voice.display_name || voice.voice_id }}
                  </option>
                </select>
              </label>
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>长度倍率 {{ previewSpeed.toFixed(2) }}（1.0 = 正常）</span>
                <input v-model.number="previewSpeed" type="range" min="0.5" max="2" step="0.05" class="accent-cyan-300" />
              </label>
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>随机度 {{ previewSdp.toFixed(2) }}</span>
                <input v-model.number="previewSdp" type="range" min="0" max="1" step="0.05" class="accent-cyan-300" />
              </label>
            </div>
          </div>

          <div class="mt-4 flex flex-wrap items-center gap-3">
            <button
              class="inline-flex min-h-9 items-center justify-center gap-[7px] rounded-md border border-cyan-300/40 bg-cyan-600/35 px-3.5 py-2 text-[13px] font-semibold text-cyan-50 transition-colors duration-200 enabled:hover:bg-cyan-600/50 disabled:cursor-not-allowed disabled:opacity-40"
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
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
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
import * as TtsLocal from '@/api/services/tts/tts-local'
import { speedToLengthScale } from '@/utils/tts/tts-speed'
import { catalogRowState } from '@/utils/tts/tts-download-state'
import type {
  CatalogAsset,
  TtsLocalInstallSnapshot,
  TtsLocalStatus,
  VoiceRecord,
} from '@/api/services/tts/tts-local'

const dialogStore = useDialogStore()
const catalog = ref<readonly CatalogAsset[]>([])
const status = ref<TtsLocalStatus | null>(null)
const snapshot = ref<TtsLocalInstallSnapshot>({ assets: [], voices: [] })
const loading = ref(false)
// 引擎初始化（加载 DeBERTa ONNX）耗时数秒，期间用黄色"加载中"提示
const engineLoading = ref(false)
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
let unlistenInstallComplete: UnlistenFn | null = null
let unlistenDownloadComplete: UnlistenFn | null = null
let componentMounted = false

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
    const [nextCatalog, nextStatus, nextSnapshot] = await Promise.all([
      TtsLocal.listCatalog(),
      TtsLocal.status(),
      TtsLocal.listInstalled(),
    ])
    catalog.value = nextCatalog
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
    audioUrl = URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' }))
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
    const switchStatus = await TtsLocal.getEnabled()
    localTtsEnabled.value = switchStatus.effective_enabled
  } catch (error) {
    notice.value = { kind: 'error', text: `读取本地 TTS 开关失败：${errorText(error)}` }
  }
}

async function saveLocalTtsSwitch(): Promise<void> {
  savingLocalTts.value = true
  try {
    // 开启时后端会同步 init 引擎（加载 DeBERTa 需数秒），期间显示"加载中"
    if (localTtsEnabled.value) engineLoading.value = true
    const switchStatus = await TtsLocal.setEnabled(localTtsEnabled.value)
    localTtsEnabled.value = switchStatus.effective_enabled
    // 开关命令会同步初始化/卸载引擎，刷新 status 让 ready 反映真实状态，
    // 否则试听区域会一直停留在旧的"未就绪"禁用态。
    await refreshAll()
    notice.value = {
      kind: 'success',
      text: localTtsEnabled.value
        ? status.value?.ready
          ? '本地 TTS 已启用。'
          : '本地 TTS 已启用，但引擎未就绪（缺少 DeBERTa 模型或分词器，请先下载）。'
        : '本地 TTS 已关闭，如需使用云端TTS，请将角色语音切换为“云端”并重启应用。',
    }
  } catch (error) {
    localTtsEnabled.value = !localTtsEnabled.value
    notice.value = { kind: 'error', text: `保存本地 TTS 开关失败：${errorText(error)}` }
  } finally {
    engineLoading.value = false
    savingLocalTts.value = false
  }
}

function rowState(assetId: string) {
  const asset = catalog.value.find((item) => item.id === assetId)
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
  componentMounted = true
  const [installComplete, downloadComplete] = await Promise.all([
    listen('tts://install-complete', () => {
      void refreshAll()
    }),
    listen('tts://download-complete', () => {
      void refreshAll()
    }),
  ])
  if (!componentMounted) {
    installComplete()
    downloadComplete()
    return
  }
  unlistenInstallComplete = installComplete
  unlistenDownloadComplete = downloadComplete

  await loadLocalTtsSwitch()
  await refreshAll()
  if (!componentMounted) return
  unlistenProgress = TtsLocal.onDownloadProgress((progress) => {
    progressByAsset.value = {
      ...progressByAsset.value,
      [progress.asset_id]: progress.percent,
    }
  })
})

onUnmounted(() => {
  componentMounted = false
  if (audioUrl) URL.revokeObjectURL(audioUrl)
  unlistenProgress?.()
  unlistenProgress = null
  unlistenInstallComplete?.()
  unlistenInstallComplete = null
  unlistenDownloadComplete?.()
  unlistenDownloadComplete = null
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

