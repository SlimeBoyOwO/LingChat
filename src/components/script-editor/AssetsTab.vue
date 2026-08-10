<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { Icon } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useI18n } from 'vue-i18n'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { AssetFile, AssetKind, AssetScope } from '@/api/services/script-editor'

const { t } = useI18n()
const store = useScriptEditorStore()

const assetKinds: { key: AssetKind; label: string }[] = [
  { key: 'background', label: t('scriptEditor.assets.kindBackground') },
  { key: 'pic', label: t('scriptEditor.assets.kindPic') },
  { key: 'music', label: t('scriptEditor.assets.kindMusic') },
  { key: 'sound', label: t('scriptEditor.assets.kindSound') },
  { key: 'ambient', label: t('scriptEditor.assets.kindAmbient') },
]

const isImageKind = (k: AssetKind) => k === 'background' || k === 'pic'

/** 绝对路径 → webview 能加载的 asset URL，与 GameBackground / GameRoleAvatar 同一套 */
const assetUrl = (path: string) => convertFileSrc(path)

const filesOf = (scope: AssetScope, kind: AssetKind): AssetFile[] =>
  store.assetFiles[scope]?.[kind] ?? []

// 音效没有全局目录（issue #6），只展示「本剧本」一列；其余素材仍是「本剧本 + 全局」
const scopesFor = (kind: AssetKind): AssetScope[] =>
  kind === 'sound' ? ['script'] : ['script', 'global']

const humanSize = (n: number) => {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

// ---- 素材音频播放速度 ----
const audioEls: Record<string, HTMLAudioElement | null> = {}
const audioRates = reactive<Record<string, number>>({})
const speedMenu = ref<string | null>(null)
const setAudioRef = (path: string) => (el: unknown) => {
  audioEls[path] = el as HTMLAudioElement | null
  if (!(path in audioRates)) audioRates[path] = 1
}
const onDocClick = () => {
  speedMenu.value = null
}
const setRate = (path: string, rate: number) => {
  const a = audioEls[path]
  if (a) {
    a.playbackRate = rate
    audioRates[path] = rate
  }
}

const IMAGE_EXT = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif']
const AUDIO_EXT = ['mp3', 'wav', 'ogg', 'flac', 'm4a']

const importAsset = async (kind: AssetKind, scope: AssetScope) => {
  const isImage = kind === 'background' || kind === 'pic'
  const picked = await openDialog({
    multiple: false,
    filters: [
      {
        name: isImage ? t('scriptEditor.fieldRow.image') : t('scriptEditor.fieldRow.audio'),
        extensions: isImage ? IMAGE_EXT : AUDIO_EXT,
      },
    ],
  })
  if (typeof picked !== 'string') return
  await store.uploadAsset(kind, scope, picked)
}

// 点击素材卡片外的区域关闭速度菜单
onMounted(() => document.addEventListener('click', onDocClick))
onUnmounted(() => document.removeEventListener('click', onDocClick))
</script>

<template>
  <MenuPage>
    <MenuItem :title="t('scriptEditor.assets.menuTitle')">
      <template #header>
        <Icon
          icon="background"
          :size="20"
        />
      </template>

      <p
        class="mb-[0.9rem]
          rounded-xl
          border
          border-white/10
          bg-black/16
          px-[0.85rem]
          py-[0.7rem]
          text-[0.76rem]
          leading-[1.85]
          text-white/60"
        v-html="t('scriptEditor.assets.hint')"
      ></p>

      <div
        v-for="k in assetKinds"
        :key="k.key"
        class="mb-[1.1rem]
          border-b
          border-white/[0.07]
          pb-[0.9rem]"
      >
        <div class="flex
          items-center
          gap-2
          mb-[0.6rem]">
          <span class="text-[0.85rem]
            font-semibold
            text-white">{{ k.label }}</span>
          <button
            class="inline-flex
              items-center
              gap-1
              border
              border-white/10
              rounded-lg
              px-3
              py-[0.3rem]
              text-[0.8rem]
              whitespace-nowrap
              text-white/70
              bg-white/6
              transition-all
              duration-200
              hover:enabled:text-white
              hover:enabled:bg-white/[0.12]
              disabled:cursor-not-allowed
              disabled:opacity-40
              ml-auto"
            @click="importAsset(k.key, 'script')"
          >
            {{ t('scriptEditor.assets.importScript') }}
          </button>
          <button
            v-if="k.key !== 'sound'"
            class="inline-flex
              items-center
              gap-1
              border
              border-white/10
              rounded-lg
              px-3
              py-[0.3rem]
              text-[0.8rem]
              whitespace-nowrap
              text-white/70
              bg-white/6
              transition-all
              duration-200
              hover:enabled:text-white
              hover:enabled:bg-white/[0.12]
              disabled:cursor-not-allowed
              disabled:opacity-40"
            @click="importAsset(k.key, 'global')"
          >
            {{ t('scriptEditor.assets.importGlobal') }}
          </button>
          <span
            v-if="k.key === 'sound'"
            class="ml-auto
              text-[0.66rem]
              text-white/40"
            >{{ t('scriptEditor.assets.soundHint') }}</span
          >
        </div>
        <div class="grid
          grid-cols-2
          gap-4
          has-[>:only-child]:grid-cols-1">
          <div
            v-for="col in scopesFor(k.key)"
            :key="col"
          >
            <p class="mb-[0.35rem]
              text-[0.7rem]
              text-white/40">
              {{
                col === 'script'
                  ? t('scriptEditor.assets.scriptScope')
                  : t('scriptEditor.assets.globalScope')
              }}
              ·
              {{ filesOf(col, k.key).length }}
            </p>
            <p
              v-if="filesOf(col, k.key).length === 0"
              class="text-[0.72rem]
                text-white/25"
            >
              {{ t('scriptEditor.assets.none') }}
            </p>
            <div
              v-for="f in filesOf(col, k.key)"
              :key="f.path"
              class="relative
                flex
                items-center
                gap-[9px]
                mb-1.5
                border
                border-white/10
                rounded-lg
                px-[9px]
                py-[7px]
                bg-white/4
                transition-all
                duration-150
                hover:border-white/[0.22]
                hover:bg-white/7
                group"
              :class="{
                [`border-purple-400/[0.22]
                bg-purple-400/7`]: col === 'global',
              }"
            >
              <!-- 图片直接出缩略图；音频给一个原生播放器，够用且零依赖 -->
              <img
                v-if="isImageKind(k.key)"
                class="asset-thumb
                  shrink-0
                  w-14
                  h-10
                  rounded-[5px]
                  object-cover"
                :src="assetUrl(f.path)"
                :alt="f.name"
                loading="lazy"
              />
              <div class="flex
                min-w-0
                flex-1
                flex-col
                gap-[3px]">
                <span
                  class="overflow-hidden
                    text-[0.74rem]
                    text-ellipsis
                    whitespace-nowrap
                    text-white/80"
                  >{{ f.name }}</span
                >
                <span class="text-[0.64rem]
                  text-white/35">{{ humanSize(f.size) }}</span>
                <div
                  v-if="!isImageKind(k.key)"
                  class="flex
                    items-center
                    gap-2"
                >
                  <audio
                    :ref="setAudioRef(f.path)"
                    class="asset-audio
                      flex-1
                      h-[26px]
                      min-w-0"
                    controls
                    preload="none"
                    controlslist="nodownload noremoteplayback"
                    :src="assetUrl(f.path)"
                  ></audio>
                  <div class="relative
                    shrink-0">
                    <button
                      class="rounded
                        px-1.5
                        py-0.5
                        text-[0.6rem]
                        border
                        border-white/10
                        text-white/40
                        hover:text-white/70
                        hover:border-white/20
                        transition-colors"
                      :title="t('scriptEditor.assets.speed')"
                      @click.stop.prevent="speedMenu = speedMenu === f.path ? null : f.path"
                    >
                      {{ audioRates[f.path] ?? 1 }}× ▾
                    </button>
                    <div
                      v-if="speedMenu === f.path"
                      class="absolute
                        bottom-full
                        right-0
                        mb-1
                        z-20
                        rounded
                        border
                        border-white/[0.14]
                        bg-[#16202c]
                        shadow-lg
                        py-0.5
                        flex
                        flex-col"
                      @click.stop
                    >
                      <button
                        v-for="rate in [0.5, 0.75, 1, 1.25, 1.5, 2]"
                        :key="rate"
                        class="px-2
                          py-0.5
                          text-[0.6rem]
                          text-left
                          whitespace-nowrap
                          transition-colors
                          hover:bg-white/10"
                        :class="(audioRates[f.path] ?? 1) === rate ? 'text-brand' : 'text-white/60'"
                        @click="
                          () => {
                            setRate(f.path, rate)
                            speedMenu = null
                          }
                        "
                      >
                        {{ rate }}×
                      </button>
                    </div>
                  </div>
                </div>
              </div>
              <button
                class="shrink-0
                  rounded
                  px-[5px]
                  text-[11px]
                  text-white/25
                  opacity-0
                  transition-all
                  duration-150
                  group-hover:opacity-100
                  hover:text-red-300
                  hover:bg-red-400/15"
                :title="t('scriptEditor.assets.delete')"
                @click="store.deleteAsset(k.key, col, f.name)"
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<style scoped>
/* 棋盘底纹：透明图片不至于糊成一片黑 */
.asset-thumb {
  background: repeating-conic-gradient(rgba(255, 255, 255, 0.08) 0% 25%, transparent 0% 50%) 0 0 /
    10px 10px;
}
/* 隐藏 Chromium 原生音频控件的「更多选项」溢出菜单（内含播放速度/循环播放）。
   速度调节已由右侧自制的 {rate}× ▾ 按钮提供，原生菜单是残留入口；
   仅在 Chromium（WebView2/Chrome）存在该伪元素，其他平台无此菜单，无副作用 */
.asset-audio::-webkit-media-controls-overflow-button {
  display: none;
}
</style>
