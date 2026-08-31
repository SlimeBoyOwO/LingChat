<template>
  <div
    class="flex h-full min-h-0 w-full flex-wrap items-start gap-5 overflow-y-auto px-3 py-6
      text-white text-shadow-2xs"
  >
    <MenuItem :title="t('settings.tts.title')" size="large">
      <template #header>
        <AudioLines :size="20" class="text-cyan-300" />
      </template>

      <div class="flex min-h-0 flex-col gap-6">
        <div class="flex flex-wrap items-center gap-x-6 gap-y-3 border-b border-white/10 pb-5">
          <label class="flex items-center gap-3">
            <input
              v-model="localTtsEnabled"
              type="checkbox"
              class="peer sr-only"
              :disabled="savingLocalTts"
              @change="saveLocalTtsSwitch"
            />
            <span
              class="relative h-5 w-9 rounded-full transition-colors"
              :class="localTtsEnabled ? 'bg-cyan-400/70' : 'bg-white/20'"
            >
              <span
                class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform"
                :class="localTtsEnabled ? 'translate-x-4' : 'translate-x-0'"
              ></span>
            </span>
            <span>
              <span class="block text-xs text-white/45">{{ t("settings.tts.switch.label") }}</span>
              <span class="block text-sm font-medium text-white">
                {{
                  localTtsEnabled
                    ? t("settings.tts.switch.enabled")
                    : t("settings.tts.switch.disabled")
                }}
              </span>
            </span>
          </label>
          <div class="h-8 w-px bg-white/10"></div>
          <div class="flex items-center gap-2">
            <span
              class="h-[9px] w-[9px] shrink-0 rounded-full"
              :class="
                engineLoading
                  ? 'bg-amber-300 shadow-[0_0_8px_rgba(252,211,77,0.5)]'
                  : status?.ready
                    ? 'bg-emerald-300 shadow-[0_0_8px_rgba(110,231,183,0.5)]'
                    : 'bg-red-400 shadow-[0_0_8px_rgba(248,113,113,0.45)]'
              "
            ></span>
            <div>
              <p class="text-xs text-white/45">{{ t("settings.tts.engine.label") }}</p>
              <p class="text-sm font-medium text-white">
                {{
                  engineLoading
                    ? t("settings.tts.engine.loading")
                    : status?.ready
                      ? t("settings.tts.engine.ready")
                      : t("settings.tts.engine.notReady")
                }}
              </p>
            </div>
          </div>
          <div class="h-8 w-px bg-white/10"></div>
          <div class="flex items-center gap-2">
            <div>
              <p class="text-xs text-white/45">{{ t("settings.tts.deberta.label") }}</p>
              <p
                class="text-sm font-medium"
                :class="status?.deberta_installed ? 'text-emerald-300' : 'text-red-300'"
              >
                {{
                  status?.deberta_installed
                    ? t("settings.tts.deberta.installed")
                    : t("settings.tts.deberta.missing")
                }}
              </p>
            </div>
            <button
              class="inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px] rounded-md
                border border-white/15 bg-white/5 text-white/80 transition-colors duration-200
                enabled:hover:border-red-400/50! enabled:hover:bg-red-400/10!
                enabled:hover:text-red-300! disabled:cursor-not-allowed disabled:opacity-40"
              :title="t('settings.tts.deberta.delete')"
              :disabled="!status?.deberta_installed || busyAction !== null"
              @click="removeDeberta"
            >
              <Trash2 :size="16" />
            </button>
          </div>
          <div class="h-8 w-px bg-white/10"></div>
          <!-- 推理设备选择：DirectML（Windows）/ WebGPU（Linux）支持 GPU；Android/macOS 只有 CPU，隐藏 -->
          <div v-if="isWindows || isLinux" class="flex items-center gap-2">
            <label class="flex flex-col">
              <span class="text-xs text-white/45">{{ t("settings.tts.device.label") }}</span>
              <select
                v-model="inferenceDevice"
                class="mt-1 rounded-md border border-white/15 bg-white/5 px-2 py-1 text-sm
                  text-white transition-colors outline-none focus:border-cyan-300/40"
                :disabled="savingDevice"
                @change="saveInferenceDevice"
              >
                <option value="cpu" class="bg-slate-800">{{ t("settings.tts.device.cpu") }}</option>
                <!-- GPU：DirectML（Windows）/ WebGPU（Linux，Dawn 默认设备）都可选 -->
                <option v-if="isWindows || isLinux" value="gpu" class="bg-slate-800">
                  {{
                    isWindows ? t("settings.tts.device.gpu") : t("settings.tts.device.gpuWebgpu")
                  }}
                </option>
                <!-- 特定显卡列表：Windows（DXGI）/ Linux（Vulkan）都枚举；其他平台无枚举 -->
                <template v-if="isWindows || isLinux">
                  <option
                    v-for="dev in gpuDevices"
                    :key="dev.id"
                    :value="`device:${dev.id}`"
                    class="bg-slate-800"
                  >
                    {{ dev.name }}
                  </option>
                </template>
              </select>
            </label>
          </div>
          <div class="h-8 w-px bg-white/10"></div>
          <div>
            <p class="text-xs text-white/45">{{ t("settings.tts.voices.label") }}</p>
            <p class="text-sm font-medium text-white">
              {{ t("settings.tts.voices.count", { count: snapshot.voices.length }) }}
            </p>
          </div>
          <button
            class="ml-auto inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px]
              rounded-md border border-white/15 bg-white/5 text-white/80 transition-colors
              duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10
              enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
            :title="t('settings.tts.refresh')"
            :disabled="loading"
            @click="refreshAll"
          >
            <RefreshCw :size="16" :class="{ 'animate-spin': loading }" />
          </button>
        </div>

        <div
          v-if="status && !status.deberta_installed"
          class="flex items-start gap-3 border-l-2 border-red-400 bg-red-500/8 px-4 py-3 text-sm
            text-red-100"
        >
          <CircleAlert :size="18" class="mt-0.5 shrink-0 text-red-300" />
          <span>{{ t("settings.tts.deberta.warning") }}</span>
        </div>

        <p
          v-if="notice"
          class="border-l-2 px-3 py-2 text-sm"
          :class="
            notice.kind === 'error'
              ? 'border-red-400 text-red-300'
              : 'border-emerald-400 text-emerald-300'
          "
        >
          {{ notice.text }}
        </p>

        <section>
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">
                {{ t("settings.tts.download.title") }}
              </h3>
              <p class="mt-0.5 text-xs text-white/45">{{ t("settings.tts.download.subtitle") }}</p>
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
                  class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px]
                    rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80
                    transition-colors duration-200 enabled:hover:border-cyan-300/40
                    enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50
                    disabled:cursor-not-allowed disabled:opacity-40"
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
              <h3 class="text-sm font-semibold text-white">{{ t("settings.tts.import.title") }}</h3>
              <p class="mt-0.5 text-xs text-white/45">{{ t("settings.tts.import.subtitle") }}</p>
            </div>
            <HardDriveDownload :size="18" class="text-white/40" />
          </div>

          <div class="flex min-w-0 gap-2">
            <input
              v-model="importVoiceId"
              class="w-full min-w-0 flex-1 rounded-md border border-white/15 bg-black/25 px-2.5 py-2
                text-[13px] text-white transition-colors outline-none focus:border-cyan-300/65
                disabled:cursor-not-allowed disabled:opacity-45"
              maxlength="64"
              :placeholder="t('settings.tts.import.voiceIdPlaceholder')"
              :aria-label="t('settings.tts.import.voiceIdPlaceholder')"
            />
            <button
              class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md
                border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80
                transition-colors duration-200 enabled:hover:border-cyan-300/40
                enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed
                disabled:opacity-40"
              :disabled="busyAction !== null"
              @click="pickVoice"
            >
              <FileArchive :size="17" />
              <span>{{ t("settings.tts.import.voice") }}</span>
            </button>
          </div>
        </section>

        <section v-if="voicesMissingStyleVectors.length > 0">
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">
                {{ t("settings.tts.styleVectors.title") }}
              </h3>
              <p class="mt-0.5 text-xs text-white/45">
                {{ t("settings.tts.styleVectors.subtitle") }}
              </p>
            </div>
            <Wand2 :size="18" class="text-white/40" />
          </div>

          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <select
              v-model="styleVectorsTarget"
              class="h-9 w-full min-w-0 flex-1 rounded-md border border-white/15 bg-black/25 px-2.5
                py-2 text-[13px] text-white transition-colors outline-none focus:border-cyan-300/65
                disabled:cursor-not-allowed disabled:opacity-45 sm:max-w-72"
              :disabled="busyAction !== null"
            >
              <option value="">{{ t("settings.tts.styleVectors.placeholder") }}</option>
              <option
                v-for="voice in voicesMissingStyleVectors"
                :key="voice.voice_id"
                :value="voice.voice_id"
              >
                {{ voice.display_name || voice.voice_id }} ({{ voice.voice_id }})
              </option>
            </select>
            <button
              class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md
                border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80
                transition-colors duration-200 enabled:hover:border-cyan-300/40
                enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50 disabled:cursor-not-allowed
                disabled:opacity-40"
              :disabled="busyAction !== null || !styleVectorsTarget"
              @click="pickStyleVectors"
            >
              <FileJson :size="17" />
              <span>{{ t("settings.tts.styleVectors.import") }}</span>
            </button>
          </div>
        </section>

        <section>
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">
                {{ t("settings.tts.installed.title") }}
              </h3>
              <p class="mt-0.5 text-xs text-white/45">
                {{ t("settings.tts.installed.subtitle", { count: snapshot.voices.length }) }}
              </p>
            </div>
            <ListMusic :size="18" class="text-white/40" />
          </div>

          <div
            v-if="snapshot.voices.length === 0"
            class="border-y border-white/10 py-[22px] text-center text-[13px] text-white/40"
          >
            {{ t("settings.tts.installed.empty") }}
          </div>
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
                  {{ voice.voice_id }} · {{ voice.kind.toUpperCase() }} ·
                  {{ formatBytes(voice.size_bytes) }}
                </p>
                <p class="mt-1 flex items-center gap-1.5 text-[11px]">
                  <span
                    v-if="voice.kind === 'sbv2'"
                    class="shrink-0 rounded border border-cyan-300/25 bg-cyan-600/10 px-1 py-px
                      text-[10px] text-cyan-50/75"
                    :title="t('settings.tts.styleVectors.builtin')"
                    >{{ t("settings.tts.styleVectors.builtin") }}</span
                  >
                  <span
                    v-else-if="voice.has_style_vectors"
                    class="shrink-0 rounded border border-cyan-300/25 bg-cyan-600/10 px-1 py-px
                      text-[10px] text-cyan-50/75"
                    :title="t('settings.tts.styleVectors.configured')"
                    >{{ t("settings.tts.styleVectors.configured") }}</span
                  >
                  <span
                    v-else
                    class="shrink-0 rounded border border-cyan-300/25 border-red-400/35!
                      bg-cyan-600/10 bg-red-400/10! px-1 py-px text-[10px] text-cyan-50/75
                      text-red-200!"
                    :title="t('settings.tts.styleVectors.missing')"
                    >{{ t("settings.tts.styleVectors.missing") }}</span
                  >
                </p>
              </div>
              <button
                class="inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px]
                  rounded-md border border-white/15 bg-white/5 text-white/80 transition-colors
                  duration-200 enabled:hover:border-cyan-300/40 enabled:hover:border-red-400/50!
                  enabled:hover:bg-cyan-300/10 enabled:hover:bg-red-400/10!
                  enabled:hover:text-cyan-50 enabled:hover:text-red-300! disabled:cursor-not-allowed
                  disabled:opacity-40"
                :title="t('settings.tts.installed.deleteVoice')"
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
              <h3 class="text-sm font-semibold text-white">
                {{ t("settings.tts.preview.title") }}
              </h3>
              <p class="mt-0.5 text-xs text-white/45">{{ t("settings.tts.preview.subtitle") }}</p>
            </div>
            <Volume2 :size="18" class="text-white/40" />
          </div>

          <div class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_17rem]">
            <textarea
              v-model="previewText"
              class="min-h-28 w-full resize-y rounded-md border border-white/15 bg-black/25 px-2.5
                py-2 text-[13px] text-white transition-colors outline-none focus:border-cyan-300/65
                disabled:cursor-not-allowed disabled:opacity-45"
              maxlength="500"
              :placeholder="t('settings.tts.preview.placeholder')"
              :disabled="!status?.ready"
            ></textarea>

            <div class="flex flex-col gap-3">
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>{{ t("settings.tts.preview.voiceModel") }}</span>
                <select
                  v-model="previewVoice"
                  class="h-9 w-full rounded-md border border-white/15 bg-black/25 px-2.5 py-2
                    text-[13px] text-white transition-colors outline-none focus:border-cyan-300/65
                    disabled:cursor-not-allowed disabled:opacity-45"
                  :disabled="!status?.ready"
                >
                  <option value="">{{ t("settings.tts.preview.select") }}</option>
                  <option
                    v-for="voice in snapshot.voices"
                    :key="voice.voice_id"
                    :value="voice.voice_id"
                  >
                    {{ voice.display_name || voice.voice_id }}
                  </option>
                </select>
              </label>
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>{{
                  t("settings.tts.preview.lengthScale", { value: previewSpeed.toFixed(2) })
                }}</span>
                <input
                  v-model.number="previewSpeed"
                  type="range"
                  min="0.5"
                  max="2"
                  step="0.05"
                  class="accent-cyan-300"
                />
              </label>
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>{{
                  t("settings.tts.preview.randomness", { value: previewSdp.toFixed(2) })
                }}</span>
                <input
                  v-model.number="previewSdp"
                  type="range"
                  min="0"
                  max="1"
                  step="0.05"
                  class="accent-cyan-300"
                />
              </label>
            </div>
          </div>

          <div class="mt-4 flex flex-wrap items-center gap-3">
            <button
              class="inline-flex min-h-9 items-center justify-center gap-[7px] rounded-md border
                border-cyan-300/40 bg-cyan-600/35 px-3.5 py-2 text-[13px] font-semibold text-cyan-50
                transition-colors duration-200 enabled:hover:bg-cyan-600/50
                disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="!canPreview || previewing"
              @click="runPreview"
            >
              <LoaderCircle v-if="previewing" :size="16" class="animate-spin" />
              <Play v-else :size="16" />
              {{
                previewing
                  ? t("settings.tts.preview.generating")
                  : t("settings.tts.preview.generate")
              }}
            </button>
            <audio ref="audioRef" controls class="h-9 min-w-0 flex-1" />
          </div>
        </section>

        <!-- 云端语音克隆 TTS（CosyVoice） -->
        <section class="border-t border-white/10 pt-6">
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h3 class="text-sm font-semibold text-white">
                {{ t("settings.tts.cosyvoice.title") }}
              </h3>
              <p class="mt-0.5 text-xs text-white/45">{{ t("settings.tts.cosyvoice.subtitle") }}</p>
            </div>
            <Mic2 :size="18" class="text-white/40" />
          </div>

          <!-- API Key -->
          <div class="mb-4 flex min-w-0 flex-wrap items-center gap-2">
            <KeyRound :size="16" class="shrink-0 text-white/40" />
            <input
              v-model="cosyKey"
              type="password"
              class="w-full min-w-0 flex-1 rounded-md border border-white/15 bg-black/25 px-2.5 py-2
                text-[13px] text-white transition-colors outline-none focus:border-cyan-300/65
                disabled:cursor-not-allowed disabled:opacity-45 sm:max-w-80"
              :placeholder="
                cosyKeyConfigured
                  ? t('settings.tts.cosyvoice.keyConfigured')
                  : t('settings.tts.cosyvoice.keyPlaceholder')
              "
              :disabled="cosyKeyConfigured"
            />
            <button
              v-if="cosyKeyConfigured"
              class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md
                border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80
                transition-colors duration-200 enabled:hover:border-cyan-300/40
                enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50"
              @click="cosyKeyConfigured = false"
            >
              <KeyRound :size="16" />
              {{ t("settings.tts.cosyvoice.keyChange") }}
            </button>
            <button
              v-else
              class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px] rounded-md
                border border-cyan-300/40 bg-cyan-600/35 px-3.5 py-2 text-[13px] font-semibold
                text-cyan-50 transition-colors duration-200 enabled:hover:bg-cyan-600/50
                disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="!cosyKey.trim()"
              @click="saveCosyKey"
            >
              {{ t("settings.tts.cosyvoice.keySave") }}
            </button>
          </div>

          <!-- 我的音色 -->
          <div class="mb-4">
            <p class="mb-2 text-xs text-white/60">
              {{ t("settings.tts.cosyvoice.voicesLabel", { count: cosyVoices.length }) }}
            </p>
            <div
              v-if="cosyVoices.length === 0"
              class="border-t border-white/10 py-[22px] text-center text-[13px] text-white/40"
            >
              {{ t("settings.tts.cosyvoice.voicesEmpty") }}
            </div>
            <div v-else class="divide-y divide-white/8 border-t border-white/10">
              <div
                v-for="voice in cosyVoices"
                :key="voice.voice_id"
                class="grid min-h-16 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 py-3"
              >
                <div class="min-w-0">
                  <p class="truncate text-sm font-medium text-white">{{ voice.name }}</p>
                  <p class="mt-1 truncate text-xs text-white/45">
                    {{ voice.voice_id }} · {{ voice.model }}
                  </p>
                  <p class="mt-1 flex items-center gap-2 text-[11px]">
                    <span :class="statusClass(voice.status)">{{ statusLabel(voice.status) }}</span>
                    <button
                      v-if="statusPaused.has(voice.voice_id)"
                      class="rounded border border-white/15 bg-white/5 px-1.5 py-px text-[10px]
                        text-white/70 transition-colors hover:border-cyan-300/40 hover:text-cyan-50"
                      @click="retryVoiceStatus(voice.voice_id)"
                    >
                      {{ t("settings.tts.cosyvoice.retryStatus") }}
                    </button>
                  </p>
                </div>
                <div class="flex shrink-0 items-center gap-2">
                  <button
                    class="inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px]
                      rounded-md border border-white/15 bg-white/5 text-white/80 transition-colors
                      duration-200 enabled:hover:border-cyan-300/40 enabled:hover:bg-cyan-300/10
                      enabled:hover:text-cyan-50 disabled:cursor-not-allowed disabled:opacity-40"
                    :title="t('settings.tts.cosyvoice.previewVoice')"
                    :disabled="cosyRegistering"
                    @click="previewVoiceFrom(voice)"
                  >
                    <Play :size="16" />
                  </button>
                  <button
                    class="inline-flex h-[34px] w-[34px] items-center justify-center gap-[7px]
                      rounded-md border border-white/15 bg-white/5 text-white/80 transition-colors
                      duration-200 enabled:hover:border-red-400/50! enabled:hover:bg-red-400/10!
                      enabled:hover:text-red-300! disabled:cursor-not-allowed disabled:opacity-40"
                    :title="t('settings.tts.cosyvoice.deleteVoice')"
                    :disabled="cosyRegistering"
                    @click="removeCosyVoice(voice)"
                  >
                    <Trash2 :size="16" />
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- 导入音色(独立小节,与试听小节结构一致):音色名称独占一行,语种/样本/注册在下方 -->
          <section class="mt-6 border-t border-white/10 pt-6">
            <div class="mb-3 flex items-center justify-between gap-3">
              <div>
                <h3 class="text-sm font-semibold text-white">
                  {{ t("settings.tts.cosyvoice.importTitle") }}
                </h3>
                <p class="mt-0.5 text-xs text-white/45">
                  {{ t("settings.tts.cosyvoice.importSubtitle") }}
                </p>
              </div>
              <FileAudio :size="18" class="text-white/40" />
            </div>

            <div class="flex min-w-0 flex-col gap-2">
              <div class="flex min-w-0 items-center gap-2">
                <span class="shrink-0 text-xs text-white/60">{{
                  t("settings.tts.cosyvoice.voiceNameLabel")
                }}</span>
                <input
                  v-model="cosyVoiceName"
                  class="h-9 w-full min-w-0 flex-1 rounded-md border border-white/15 bg-black/25
                    px-2.5 py-2 text-[13px] text-white transition-colors outline-none
                    focus:border-cyan-300/65 disabled:cursor-not-allowed disabled:opacity-45"
                  maxlength="32"
                  :placeholder="t('settings.tts.cosyvoice.voiceNamePlaceholder')"
                  :disabled="!cosyKeyConfigured || cosyRegistering"
                />
              </div>
              <div class="flex min-w-0 flex-wrap items-center gap-2">
                <span class="shrink-0 text-xs text-white/60">{{
                  t("settings.tts.cosyvoice.languageLabel")
                }}</span>
                <select
                  v-model="cosyLang"
                  class="h-9 rounded-md border border-white/15 bg-black/25 px-2.5 py-2 text-[13px]
                    text-white transition-colors outline-none focus:border-cyan-300/65
                    disabled:cursor-not-allowed disabled:opacity-45"
                  :disabled="!cosyKeyConfigured || cosyRegistering"
                >
                  <option
                    v-for="lang in COSYVOICE_LANGUAGES"
                    :key="lang"
                    :value="lang"
                    class="bg-slate-800"
                  >
                    {{ t(`settings.tts.cosyvoice.languages.${lang}`) }}
                  </option>
                </select>
                <button
                  class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px]
                    rounded-md border border-white/15 bg-white/5 px-3 py-2 text-[13px] text-white/80
                    transition-colors duration-200 enabled:hover:border-cyan-300/40
                    enabled:hover:bg-cyan-300/10 enabled:hover:text-cyan-50
                    disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="!cosyKeyConfigured || cosyRegistering"
                  @click="pickCosySample"
                >
                  <FileArchive :size="17" />
                  <span>
                    {{
                      cosySamplePath
                        ? cosySamplePath.split(/[\\/]/).pop()
                        : t("settings.tts.cosyvoice.pickSample")
                    }}
                  </span>
                </button>
                <button
                  class="inline-flex min-h-9 shrink-0 items-center justify-center gap-[7px]
                    rounded-md border border-cyan-300/40 bg-cyan-600/35 px-3.5 py-2 text-[13px]
                    font-semibold text-cyan-50 transition-colors duration-200
                    enabled:hover:bg-cyan-600/50 disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="!cosyKeyConfigured || cosyRegistering || !cosySamplePath"
                  @click="registerCosyVoice()"
                >
                  <LoaderCircle v-if="cosyRegistering" :size="16" class="animate-spin" />
                  <Mic2 v-else :size="16" />
                  <span>
                    {{
                      cosyRegistering
                        ? t("settings.tts.cosyvoice.registering")
                        : t("settings.tts.cosyvoice.register")
                    }}
                  </span>
                </button>
              </div>
            </div>

            <!-- 导入音频要求提示(单行) -->
            <p class="mt-3 border-l border-white/10 pl-3 text-xs leading-5 text-white/45">
              {{ t("settings.tts.cosyvoice.importHint") }}
            </p>
          </section>

          <!-- 试听(独立小节,与内置 TTS 试听结构一致) -->
          <section class="mt-6 border-t border-white/10 pt-6">
            <div class="mb-3 flex items-center justify-between gap-3">
              <div>
                <h3 class="text-sm font-semibold text-white">
                  {{ t("settings.tts.cosyvoice.previewTitle") }}
                </h3>
                <p class="mt-0.5 text-xs text-white/45">
                  {{ t("settings.tts.cosyvoice.previewSubtitle") }}
                </p>
              </div>
              <Volume2 :size="18" class="text-white/40" />
            </div>

            <div class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_17rem]">
              <textarea
                v-model="previewText"
                class="min-h-28 w-full resize-y rounded-md border border-white/15 bg-black/25 px-2.5
                  py-2 text-[13px] text-white transition-colors outline-none
                  focus:border-cyan-300/65 disabled:cursor-not-allowed disabled:opacity-45"
                maxlength="500"
                :placeholder="t('settings.tts.cosyvoice.previewPlaceholder')"
                :disabled="!cosyKeyConfigured"
              ></textarea>
              <label class="flex flex-col gap-1.5 text-xs text-white/60">
                <span>{{ t("settings.tts.cosyvoice.previewVoiceLabel") }}</span>
                <select
                  v-model="cosyPreviewVoice"
                  class="h-9 w-full rounded-md border border-white/15 bg-black/25 px-2.5 py-2
                    text-[13px] text-white transition-colors outline-none focus:border-cyan-300/65
                    disabled:cursor-not-allowed disabled:opacity-45"
                  :disabled="!cosyKeyConfigured"
                >
                  <option value="">{{ t("settings.tts.cosyvoice.previewSelect") }}</option>
                  <option
                    v-for="voice in cosyVoices"
                    :key="voice.voice_id"
                    :value="voice.voice_id"
                    class="bg-slate-800"
                  >
                    {{ voice.name }}
                  </option>
                </select>
              </label>
            </div>
            <div class="mt-4 flex flex-wrap items-center gap-3">
              <button
                class="inline-flex min-h-9 items-center justify-center gap-[7px] rounded-md border
                  border-cyan-300/40 bg-cyan-600/35 px-3.5 py-2 text-[13px] font-semibold
                  text-cyan-50 transition-colors duration-200 enabled:hover:bg-cyan-600/50
                  disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="!cosyKeyConfigured || !cosyPreviewVoice || cosyPreviewing"
                @click="runCosyPreview"
              >
                <LoaderCircle v-if="cosyPreviewing" :size="16" class="animate-spin" />
                <Play v-else :size="16" />
                {{
                  cosyPreviewing
                    ? t("settings.tts.cosyvoice.generating")
                    : t("settings.tts.cosyvoice.generate")
                }}
              </button>
              <audio ref="cosyAudioRef" controls class="h-9 min-w-0 flex-1" />
            </div>
          </section>
        </section>
      </div>
    </MenuItem>
  </div>
</template>

<script setup lang="ts">
  import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { DialogFilter } from "@tauri-apps/plugin-dialog";
  import {
    AudioLines,
    Check,
    CircleAlert,
    FileArchive,
    FileAudio,
    FileDown,
    FileJson,
    HardDriveDownload,
    KeyRound,
    ListMusic,
    LoaderCircle,
    Mic2,
    Play,
    RefreshCw,
    Trash2,
    Volume2,
    Wand2,
  } from "lucide-vue-next";
  import { MenuItem } from "@/components/ui";
  import { useDialogStore } from "@/stores/modules/ui/dialog";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import * as TtsLocal from "@/api/services/tts/tts-local";
  import * as TtsCosyvoice from "@/api/services/tts/tts-cosyvoice";
  import { speedToLengthScale } from "@/utils/tts/tts-speed";
  import { catalogRowState } from "@/utils/tts/tts-download-state";
  import type {
    CatalogAsset,
    TtsLocalInstallSnapshot,
    TtsLocalStatus,
    VoiceRecord,
  } from "@/api/services/tts/tts-local";

  const dialogStore = useDialogStore();
  const uiStore = useUIStore();
  const { t } = useI18n();
  const catalog = ref<readonly CatalogAsset[]>([]);
  const status = ref<TtsLocalStatus | null>(null);
  const snapshot = ref<TtsLocalInstallSnapshot>({ assets: [], voices: [] });
  const loading = ref(false);
  // 引擎初始化（加载 DeBERTa ONNX）耗时数秒，期间用黄色"加载中"提示
  const engineLoading = ref(false);
  const busyAction = ref<string | null>(null);
  const importVoiceId = ref("");
  const styleVectorsTarget = ref("");
  const notice = ref<{ kind: "success" | "error"; text: string } | null>(null);
  const previewText = ref("こんにちは、これはローカル音声のテストです。");
  const previewVoice = ref("");
  const previewSpeed = ref(1);
  const previewSdp = ref(0);
  const previewing = ref(false);
  const audioRef = ref<HTMLAudioElement | null>(null);
  let audioUrl: string | null = null;
  // cosyvoice 试听独立 audio 元素/URL（与本地 TTS 试听分开，避免同名 ref 互相覆盖）
  const cosyAudioRef = ref<HTMLAudioElement | null>(null);
  let cosyAudioUrl: string | null = null;
  const progressByAsset = ref<Record<string, number>>({});
  const downloadError = ref<Record<string, string>>({});
  const downloadingId = ref<string | null>(null);
  const localTtsEnabled = ref(false);
  const savingLocalTts = ref(false);
  // CosyVoice 云端语音克隆
  const cosyKey = ref("");
  const cosyKeyConfigured = ref(false);
  const cosyModels = ref<string[]>([]);
  const cosyVoices = ref<TtsCosyvoice.CosyVoiceView[]>([]);
  const cosyVoiceName = ref("");
  const cosyLang = ref("zh");
  const cosySamplePath = ref("");
  const cosyRegistering = ref(false);
  const cosyPreviewVoice = ref("");
  const cosyPreviewing = ref(false);
  // 参考音频语言(cosyvoice-v3.5-flash 的 language_hints 官方支持范围)
  const COSYVOICE_LANGUAGES = [
    "zh",
    "en",
    "ja",
    "ko",
    "fr",
    "de",
    "ru",
    "pt",
    "th",
    "id",
    "vi",
  ] as const;
  // 云端合成固定使用音色所属模型(注册模型必须与之一致);列表为空时回退官方默认
  const DEFAULT_COSYVOICE_MODEL = "cosyvoice-v3.5-flash";

  function voiceModelOf(voiceId: string): string {
    const voice = cosyVoices.value.find((v) => v.voice_id === voiceId);
    return voice?.model || cosyModels.value[0] || DEFAULT_COSYVOICE_MODEL;
  }
  // 审核状态轮询(参考 N.E.K.O.:5s 周期 + 在途/暂停去重 + 失败暂停手动重查)
  const statusPolling = new Set<string>();
  const statusPaused = new Set<string>();
  let statusTimer: ReturnType<typeof setInterval> | null = null;

  // 终态(不再轮询)
  function isTerminalStatus(status: string | null | undefined): boolean {
    const s = status?.toLowerCase();
    return s === "ok" || s === "undeployed";
  }

  // 状态徽标样式
  function statusClass(status: string | null | undefined): string {
    switch (status?.toLowerCase()) {
      case "ok":
        return "text-emerald-300";
      case "undeployed":
        return "text-red-300";
      case "deploying":
        return "text-amber-300";
      default:
        return "text-white/40";
    }
  }

  function statusLabel(status: string | null | undefined): string {
    switch (status?.toLowerCase()) {
      case "ok":
        return t("settings.tts.cosyvoice.statusOk");
      case "undeployed":
        return t("settings.tts.cosyvoice.statusUndeployed");
      case "deploying":
        return t("settings.tts.cosyvoice.statusDeploying");
      default:
        return t("settings.tts.cosyvoice.statusUnknown");
    }
  }

  // 查单个音色状态:终态刷新列表;失败加入暂停集(显示"状态未知"+手动重查)
  async function pollVoiceStatus(voiceId: string): Promise<void> {
    statusPolling.add(voiceId);
    try {
      const status = await TtsCosyvoice.voiceStatus(voiceId);
      if (isTerminalStatus(status)) {
        await loadCosyvoice();
      }
      // 非终态:不刷新,下个 5s 周期自动重新加入再查
    } catch {
      statusPaused.add(voiceId);
      await loadCosyvoice();
    } finally {
      statusPolling.delete(voiceId);
    }
  }

  // 每周期找出需要查的音色:非终态 + 不在途 + 未暂停
  function startStatusPolling(): void {
    for (const voice of cosyVoices.value) {
      if (
        isTerminalStatus(voice.status) ||
        statusPolling.has(voice.voice_id) ||
        statusPaused.has(voice.voice_id)
      ) {
        continue;
      }
      void pollVoiceStatus(voice.voice_id);
    }
  }

  // 手动重查:解除暂停并立即查一次
  async function retryVoiceStatus(voiceId: string): Promise<void> {
    statusPaused.delete(voiceId);
    await pollVoiceStatus(voiceId);
  }

  function stopStatusPolling(): void {
    if (statusTimer) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  }
  // 推理设备（本地 TTS 热切换）：仅 Windows 显示 GPU 选项
  const inferenceDevice = ref("cpu");
  const savingDevice = ref(false);
  const isWindows = /win32|windows/i.test(navigator.userAgent);
  // 安卓 WebView 的 UA 也含 "Linux"，需排除（安卓无 GPU 推理后端）
  const isLinux = /linux/i.test(navigator.userAgent) && !/android/i.test(navigator.userAgent);
  // DirectML GPU 列表（device:<id> 选项）
  const gpuDevices = ref<{ id: number; name: string }[]>([]);
  let unlistenProgress: (() => void) | null = null;
  let unlistenInstallComplete: UnlistenFn | null = null;
  let unlistenDownloadComplete: UnlistenFn | null = null;
  let unlistenStatusChanged: UnlistenFn | null = null;
  let componentMounted = false;

  async function saveInferenceDevice() {
    savingDevice.value = true;
    try {
      await TtsLocal.setDevice(inferenceDevice.value);
      // 提示用型号名而非 device:<id>
      const dev = gpuDevices.value.find((d) => `device:${d.id}` === inferenceDevice.value);
      notice.value = {
        kind: "success",
        text: t("settings.tts.messages.deviceSwitched", {
          name: dev ? dev.name : inferenceDevice.value,
        }),
      };
    } catch (e) {
      console.error("切换推理设备失败:", e);
      notice.value = {
        kind: "error",
        text: t("settings.tts.messages.deviceSwitchFailed", { error: errorText(e) }),
      };
      // 失败时回滚下拉显示
      inferenceDevice.value = "cpu";
    } finally {
      savingDevice.value = false;
    }
  }

  type FilterIntent = "voice" | "style_vectors";

  // Android plugin-dialog interprets the `extensions` field as MIME types
  // (not file extensions). ONNX / SBV2 have no registered MIME, so they fall
  // back to application/octet-stream; the backend validates the actual file
  // via archive::inspect_package and rejects unknown formats.
  function dialogFilters(intent: FilterIntent): DialogFilter[] {
    if (/android/i.test(navigator.userAgent)) {
      switch (intent) {
        case "voice":
          return [{ name: "Voice model", extensions: ["application/octet-stream"] }];
        case "style_vectors":
          return [{ name: "style_vectors JSON", extensions: ["application/json", "text/json"] }];
      }
    }
    switch (intent) {
      case "voice":
        return [{ name: "SBV2 voice", extensions: ["sbv2", "onnx"] }];
      case "style_vectors":
        return [{ name: "style_vectors JSON", extensions: ["json"] }];
    }
  }

  const canPreview = computed(() =>
    Boolean(status.value?.ready && previewVoice.value && previewText.value.trim())
  );

  const voicesMissingStyleVectors = computed(() =>
    snapshot.value.voices.filter((voice) => voice.kind === "onnx" && !voice.has_style_vectors)
  );

  function errorText(error: unknown): string {
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    return JSON.stringify(error);
  }

  function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    return `${(bytes / 1024 ** index).toFixed(index >= 2 ? 1 : 0)} ${units[index]}`;
  }

  function selectedPath(value: string | string[] | null): string | null {
    if (typeof value === "string") return value;
    return value?.[0] ?? null;
  }

  function normalizeVoiceId(value: string): string {
    const fileName =
      value
        .split(/[\\/]/)
        .pop()
        ?.replace(/\.(sbv2|onnx)$/i, "") || "local-voice";
    const normalized = fileName
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return (normalized || "local-voice").slice(0, 64);
  }

  async function refreshAll(): Promise<void> {
    loading.value = true;
    try {
      const [nextCatalog, nextStatus, nextSnapshot] = await Promise.all([
        TtsLocal.listCatalog(),
        TtsLocal.status(),
        TtsLocal.listInstalled(),
      ]);
      catalog.value = nextCatalog;
      status.value = nextStatus;
      snapshot.value = nextSnapshot;
      if (
        !previewVoice.value ||
        !nextSnapshot.voices.some((voice) => voice.voice_id === previewVoice.value)
      ) {
        previewVoice.value = nextSnapshot.voices[0]?.voice_id ?? "";
      }
      await loadCosyvoice();
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.readStatusFailed", { error: errorText(error) })}`,
      };
    } finally {
      loading.value = false;
    }
  }

  async function pickVoice(): Promise<void> {
    const selection = await open({
      multiple: false,
      filters: dialogFilters("voice"),
    });
    const path = selectedPath(selection);
    if (!path) return;

    busyAction.value = "import:voice";
    notice.value = null;
    try {
      const voiceId = normalizeVoiceId(importVoiceId.value.trim() || path);
      await TtsLocal.importFromPath(path, voiceId);
      importVoiceId.value = "";
      notice.value = {
        kind: "success",
        text: `${t("settings.tts.messages.importVoiceSuccess", { voiceId })}`,
      };
      await refreshAll();
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.importFailed", { error: errorText(error) })}`,
      };
    } finally {
      busyAction.value = null;
    }
  }

  async function pickStyleVectors(): Promise<void> {
    if (!styleVectorsTarget.value) {
      notice.value = { kind: "error", text: t("settings.tts.messages.styleVectorsNeedSelect") };
      return;
    }
    const selection = await open({
      multiple: false,
      filters: dialogFilters("style_vectors"),
    });
    const path = selectedPath(selection);
    if (!path) return;

    const target = styleVectorsTarget.value;
    busyAction.value = `style-vectors:${target}`;
    notice.value = null;
    try {
      await TtsLocal.importStyleVectors(target, path);
      notice.value = {
        kind: "success",
        text: `${t("settings.tts.messages.styleVectorsSuccess", { target })}`,
      };
      await refreshAll();
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.importFailed", { error: errorText(error) })}`,
      };
    } finally {
      busyAction.value = null;
    }
  }

  async function removeVoice(voice: VoiceRecord): Promise<void> {
    const confirmed = await dialogStore.confirm(
      `${t("settings.tts.messages.deleteConfirm", { name: voice.display_name || voice.voice_id })}`,
      t("settings.tts.messages.deleteConfirmTitle")
    );
    if (!confirmed) return;

    busyAction.value = `delete:${voice.voice_id}`;
    notice.value = null;
    try {
      await TtsLocal.deleteVoice(voice.voice_id);
      notice.value = { kind: "success", text: t("settings.tts.messages.deleteSuccess") };
      await refreshAll();
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.deleteFailed", { error: errorText(error) })}`,
      };
    } finally {
      busyAction.value = null;
    }
  }

  async function removeDeberta(): Promise<void> {
    const confirmed = await dialogStore.confirm(
      t("settings.tts.messages.deleteDebertaConfirm"),
      t("settings.tts.messages.deleteDebertaConfirmTitle")
    );
    if (!confirmed) return;

    busyAction.value = "delete:deberta";
    notice.value = null;
    try {
      await TtsLocal.deleteDeberta();
      notice.value = { kind: "success", text: t("settings.tts.messages.deleteDebertaSuccess") };
      await refreshAll();
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.deleteDebertaFailed", { error: errorText(error) })}`,
      };
    } finally {
      busyAction.value = null;
    }
  }

  // Tauri invoke 返回 Vec<u8> 的三种形态:ArrayBuffer(自定义协议 Raw 路径)/
  // Uint8Array / number[](postMessage fallback 序列化成 JSON 数组)。
  // 统一归一化成 ArrayBuffer 才能正确构造 Blob——直接 Blob([number[]]) 会被
  // String() 成逗号字符串,播放必然失败。
  function toAudioBuffer(bytes: Uint8Array | ArrayBuffer | number[]): ArrayBuffer {
    if (bytes instanceof ArrayBuffer) return bytes;
    if (ArrayBuffer.isView(bytes)) {
      // TypedArray/DataView 可能是更大 ArrayBuffer 的部分视图(byteOffset/byteLength 限定),
      // 直接返回 bytes.buffer 会带上视图外的无关字节,必须切出精确区间。
      // Tauri invoke 返回的底层 buffer 实际一定是 ArrayBuffer(非 SharedArrayBuffer),断言安全
      return bytes.buffer.slice(
        bytes.byteOffset,
        bytes.byteOffset + bytes.byteLength
      ) as ArrayBuffer;
    }
    return new Uint8Array(bytes).buffer;
  }

  async function runPreview(): Promise<void> {
    if (!canPreview.value) return;
    previewing.value = true;
    notice.value = null;
    try {
      const bytes = await TtsLocal.synthesizePreview({
        text: previewText.value.trim(),
        voiceId: previewVoice.value,
        lengthScale: speedToLengthScale(previewSpeed.value),
        sdpRatio: previewSdp.value,
      });
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(new Blob([toAudioBuffer(bytes)], { type: "audio/wav" }));
      await nextTick();
      if (audioRef.value) {
        audioRef.value.src = audioUrl;
        await audioRef.value.play();
      }
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.previewFailed", { error: errorText(error) })}`,
      };
    } finally {
      previewing.value = false;
    }
  }

  async function loadLocalTtsSwitch(): Promise<void> {
    try {
      const switchStatus = await TtsLocal.getEnabled();
      localTtsEnabled.value = switchStatus.effective_enabled;
    } catch (error) {
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.readSwitchFailed", { error: errorText(error) })}`,
      };
    }
  }

  async function saveLocalTtsSwitch(): Promise<void> {
    savingLocalTts.value = true;
    try {
      // 开启时后端会同步 init 引擎（加载 DeBERTa 需数秒），期间显示"加载中"
      if (localTtsEnabled.value) engineLoading.value = true;
      const switchStatus = await TtsLocal.setEnabled(localTtsEnabled.value);
      localTtsEnabled.value = switchStatus.effective_enabled;
      // 开关命令会同步初始化/卸载引擎，刷新 status 让 ready 反映真实状态，
      // 否则试听区域会一直停留在旧的"未就绪"禁用态。
      await refreshAll();
      if (localTtsEnabled.value) {
        // 开启成功：左上角弹窗引导去角色语音设置切换 TTS 类型
        uiStore.showNotification({
          type: "info",
          title: t("settings.tts.messages.enableHintTitle"),
          message: t("settings.tts.messages.enableHintMessage"),
          duration: 6000,
          skipTipsCheck: true,
        });
      }
      notice.value = {
        kind: "success",
        text: localTtsEnabled.value
          ? status.value?.ready
            ? t("settings.tts.messages.switchEnabled")
            : t("settings.tts.messages.switchEnabledNotReady")
          : t("settings.tts.messages.switchDisabled"),
      };
    } catch (error) {
      localTtsEnabled.value = !localTtsEnabled.value;
      notice.value = {
        kind: "error",
        text: `${t("settings.tts.messages.saveSwitchFailed", { error: errorText(error) })}`,
      };
    } finally {
      engineLoading.value = false;
      savingLocalTts.value = false;
    }
  }

  function rowState(assetId: string) {
    const asset = catalog.value.find((item) => item.id === assetId);
    if (!asset) return "missing";
    return catalogRowState({
      asset,
      progressPercent: progressByAsset.value[assetId],
      errorMessage: downloadError.value[assetId],
      status: status.value,
      voices: snapshot.value.voices,
    });
  }

  function rowLabel(assetId: string): string {
    const state = rowState(assetId);
    if (state === "installed") return t("settings.tts.download.installed");
    if (state === "downloading") return t("settings.tts.download.downloading");
    if (state === "error") return t("settings.tts.download.retry");
    return t("settings.tts.download.start");
  }

  watch(
    () => snapshot.value.voices,
    (voices) => {
      if (!voices.some((voice) => voice.voice_id === previewVoice.value)) {
        previewVoice.value = voices[0]?.voice_id ?? "";
      }
      if (
        styleVectorsTarget.value &&
        !voices.some((voice) => voice.voice_id === styleVectorsTarget.value)
      ) {
        styleVectorsTarget.value = "";
      }
    }
  );

  onMounted(async () => {
    componentMounted = true;
    const [installComplete, downloadComplete, statusChanged] = await Promise.all([
      listen("tts://install-complete", () => {
        void refreshAll();
      }),
      listen("tts://download-complete", () => {
        void refreshAll();
      }),
      // 历史页「生成语音」触发：生成前后端会广播，静默刷新引擎就绪状态
      listen("tts://status-changed", () => {
        void refreshAll();
      }),
    ]);
    if (!componentMounted) {
      installComplete();
      downloadComplete();
      statusChanged();
      return;
    }
    unlistenInstallComplete = installComplete;
    unlistenDownloadComplete = downloadComplete;
    unlistenStatusChanged = statusChanged;

    await loadLocalTtsSwitch();
    await refreshAll();
    await loadCosyvoice();
    // 审核状态轮询(每 5s;终态自动停止,失败自动暂停)
    statusTimer = setInterval(startStatusPolling, 5000);
    startStatusPolling();

    // 加载 GPU 设备列表（Windows 用 DXGI，Linux 用 Vulkan 枚举特定显卡）
    if (isWindows || isLinux) {
      try {
        const devices = await TtsLocal.listDevices();
        gpuDevices.value = devices.map((d) => ({ id: d.id, name: d.name }));
      } catch (e) {
        console.error("枚举推理设备失败:", e);
      }
    }

    // 读取当前推理设备（持久化配置），同步下拉框显示
    try {
      const current = await TtsLocal.getDevice();
      if (current) inferenceDevice.value = current;
    } catch (e) {
      console.error("读取推理设备失败:", e);
    }
    if (!componentMounted) return;
    unlistenProgress = TtsLocal.onDownloadProgress((progress) => {
      progressByAsset.value = {
        ...progressByAsset.value,
        [progress.asset_id]: progress.percent,
      };
    });
  });

  onUnmounted(() => {
    componentMounted = false;
    stopStatusPolling();
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    if (cosyAudioUrl) URL.revokeObjectURL(cosyAudioUrl);
    unlistenProgress?.();
    unlistenProgress = null;
    unlistenInstallComplete?.();
    unlistenInstallComplete = null;
    unlistenDownloadComplete?.();
    unlistenDownloadComplete = null;
    unlistenStatusChanged?.();
    unlistenStatusChanged = null;
  });

  async function loadCosyvoice(): Promise<void> {
    try {
      const cfg = await TtsCosyvoice.getConfig();
      cosyKeyConfigured.value = cfg.api_key_configured;
      cosyModels.value = cfg.models;
      const voices = await TtsCosyvoice.listVoices();
      cosyVoices.value = voices;
      if (!cosyPreviewVoice.value || !voices.some((v) => v.voice_id === cosyPreviewVoice.value)) {
        cosyPreviewVoice.value = voices[0]?.voice_id ?? "";
      }
    } catch (e) {
      notice.value = {
        kind: "error",
        text: t("settings.tts.cosyvoice.notices.loadConfigFailed", { error: errorText(e) }),
      };
    }
  }

  async function saveCosyKey(): Promise<void> {
    const key = cosyKey.value.trim();
    if (!key) return;
    try {
      await TtsCosyvoice.saveApiKey(key);
      cosyKey.value = "";
      cosyKeyConfigured.value = true;
      notice.value = { kind: "success", text: t("settings.tts.cosyvoice.notices.keySaved") };
      await loadCosyvoice();
    } catch (e) {
      notice.value = {
        kind: "error",
        text: t("settings.tts.cosyvoice.notices.keySaveFailed", { error: errorText(e) }),
      };
    }
  }

  async function pickCosySample(): Promise<void> {
    // Android 上 plugin-dialog 的 extensions 字段是 MIME 类型(非扩展名),需特判
    const filters = /android/i.test(navigator.userAgent)
      ? [{ name: "Audio sample", extensions: ["audio/wav", "audio/mpeg", "audio/flac"] }]
      : [{ name: "Audio sample", extensions: ["wav", "mp3", "flac"] }];
    const selection = await open({ multiple: false, filters });
    const path = selectedPath(selection);
    if (path) cosySamplePath.value = path;
  }

  async function registerCosyVoice(): Promise<void> {
    const name = cosyVoiceName.value.trim();
    if (!name || !cosySamplePath.value) {
      notice.value = { kind: "error", text: t("settings.tts.cosyvoice.notices.needNameAndSample") };
      return;
    }
    cosyRegistering.value = true;
    notice.value = null;
    try {
      const record = await TtsCosyvoice.createVoice(
        name,
        cosyModels.value[0] || DEFAULT_COSYVOICE_MODEL,
        cosySamplePath.value,
        cosyLang.value,
        (phase) => {
          // 后端 progress 传 phase key（uploading/submitting/submitted），此处映射本地化文案
          const phaseText = t(`settings.tts.cosyvoice.phases.${phase}`);
          notice.value = { kind: "success", text: phaseText };
        }
      );
      notice.value = {
        kind: "success",
        text: t("settings.tts.cosyvoice.notices.voiceSubmitted", {
          name: record.name,
          voiceId: record.voice_id,
        }),
      };
      cosyVoiceName.value = "";
      cosySamplePath.value = "";
      await loadCosyvoice();
      startStatusPolling();
    } catch (e) {
      notice.value = {
        kind: "error",
        text: t("settings.tts.cosyvoice.notices.registerFailed", { error: errorText(e) }),
      };
    } finally {
      cosyRegistering.value = false;
    }
  }

  async function removeCosyVoice(voice: TtsCosyvoice.CosyVoiceView): Promise<void> {
    const confirmed = await dialogStore.confirm(
      t("settings.tts.cosyvoice.notices.deleteConfirm", { name: voice.name }),
      t("settings.tts.cosyvoice.notices.deleteTitle")
    );
    if (!confirmed) return;
    try {
      await TtsCosyvoice.deleteVoice(voice.voice_id);
      await loadCosyvoice();
    } catch (e) {
      notice.value = {
        kind: "error",
        text: t("settings.tts.cosyvoice.notices.deleteFailed", { error: errorText(e) }),
      };
    }
  }

  // 音色卡片的试听:合成模型取音色自己所属的模型(必须与注册模型一致,否则云端报错)
  async function previewVoiceFrom(voice: TtsCosyvoice.CosyVoiceView): Promise<void> {
    cosyPreviewVoice.value = voice.voice_id;
    await runCosyPreview();
  }

  async function runCosyPreview(): Promise<void> {
    if (!cosyPreviewVoice.value) return;
    if (!previewText.value.trim()) {
      notice.value = { kind: "error", text: t("settings.tts.cosyvoice.notices.needPreviewText") };
      return;
    }
    console.log("[cosyvoice] 试听开始", {
      model: voiceModelOf(cosyPreviewVoice.value),
      voiceId: cosyPreviewVoice.value,
      textLen: previewText.value.trim().length,
    });
    cosyPreviewing.value = true;
    notice.value = null;
    try {
      const bytes = await TtsCosyvoice.synthesizePreview(
        voiceModelOf(cosyPreviewVoice.value),
        cosyPreviewVoice.value,
        previewText.value.trim()
      );
      console.log(
        "[cosyvoice] 合成返回",
        bytes?.byteLength ?? "unknown",
        "bytes, type:",
        Object.prototype.toString.call(bytes)
      );
      // 统一归一化成 ArrayBuffer(见 toAudioBuffer 注释),否则 Blob 内容会变成逗号字符串
      const audioBuffer = toAudioBuffer(bytes);
      console.log("[cosyvoice] Blob 字节数", audioBuffer.byteLength);
      if (cosyAudioUrl) URL.revokeObjectURL(cosyAudioUrl);
      cosyAudioUrl = URL.createObjectURL(new Blob([audioBuffer], { type: "audio/wav" }));
      await nextTick();
      if (cosyAudioRef.value) {
        cosyAudioRef.value.src = cosyAudioUrl;
        try {
          await cosyAudioRef.value.play();
          console.log("[cosyvoice] 播放中", cosyAudioUrl);
        } catch (playErr) {
          console.error("[cosyvoice] 播放失败", playErr);
          throw playErr;
        }
      }
    } catch (e) {
      console.error("[cosyvoice] 试听失败", e);
      notice.value = {
        kind: "error",
        text: t("settings.tts.cosyvoice.notices.previewFailed", { error: errorText(e) }),
      };
    } finally {
      cosyPreviewing.value = false;
    }
  }

  async function triggerDownload(assetId: string): Promise<void> {
    if (downloadingId.value) return;
    downloadingId.value = assetId;
    const nextProgress = { ...progressByAsset.value };
    delete nextProgress[assetId];
    progressByAsset.value = nextProgress;
    const nextErrors = { ...downloadError.value };
    delete nextErrors[assetId];
    downloadError.value = nextErrors;
    try {
      await TtsLocal.download(assetId);
      await refreshAll();
      const completedProgress = { ...progressByAsset.value };
      completedProgress[assetId] = 100;
      progressByAsset.value = completedProgress;
    } catch (error) {
      downloadError.value = {
        ...downloadError.value,
        [assetId]: errorText(error),
      };
    } finally {
      downloadingId.value = null;
    }
  }
</script>
