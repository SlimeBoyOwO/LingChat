<template>
  <div class="flex flex-col gap-5 p-2">
    <!-- 描述 -->
    <div class="text-xs text-white/50 leading-relaxed">
      {{ $t('settings.background.dialog.description') }}
    </div>

    <!-- 自定义背景图 -->
    <div class="flex flex-col gap-2">
      <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.backgroundImage') }}</label>
      <div class="flex items-center gap-3">
        <div
          class="w-20 h-12 rounded-md border border-white/10 bg-white/5 flex items-center justify-center overflow-hidden"
        >
          <img
            v-if="dialogBackgroundImage"
            :src="dialogBackgroundImage"
            class="w-full h-full object-cover"
            alt="dialog bg"
          />
          <span v-else class="text-white/30 text-xs">{{ $t('settings.background.dialog.noImage') }}</span>
        </div>
        <input
          ref="dialogBgInput"
          type="file"
          accept="image/*"
          class="hidden"
          @change="handleDialogBgUpload"
        />
        <button
          class="px-4 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-brand/80 border-brand text-white hover:bg-brand shadow-indigo-500/20"
          @click="triggerDialogBgUpload"
        >
          <Upload :size="14" class="inline-block mr-1" />
          {{ dialogBackgroundImage ? $t('settings.background.dialog.change') : $t('settings.background.dialog.upload') }}
        </button>
        <button
          v-if="dialogBackgroundImage"
          class="px-3 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-white/10 border-white/20 text-white/70 hover:bg-white/20"
          @click="clearDialogBackgroundImage"
        >
          {{ $t('settings.background.dialog.clear') }}
        </button>
        <span class="ml-auto text-white/40 text-xs">{{ $t('settings.background.dialog.sizeHint') }}</span>
      </div>
    </div>

    <!-- 背景透明度 -->
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.opacity') }}</label>
        <span class="text-white/50 text-xs">{{ dialogOpacityPercent }}%</span>
      </div>
      <Slider
        :min="0"
        :max="100"
        :step="1"
        :model-value="dialogOpacityPercent"
        @update:model-value="(v: number) => (dialogOpacityPercent = v)"
      />
    </div>

    <!-- 背景模糊 -->
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.blur') }}</label>
        <span class="text-white/50 text-xs">{{ dialogBlur }}px</span>
      </div>
      <Slider
        :min="0"
        :max="40"
        :step="1"
        :model-value="dialogBlur"
        @update:model-value="(v: number) => (dialogBlur = v)"
      />
    </div>

    <!-- 圆角大小 -->
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.borderRadius') }}</label>
        <span class="text-white/50 text-xs">{{ dialogBorderRadius }}px</span>
      </div>
      <Slider
        :min="0"
        :max="40"
        :step="1"
        :model-value="dialogBorderRadius"
        @update:model-value="(v: number) => (dialogBorderRadius = v)"
      />
    </div>

    <!-- 渐变底色 -->
    <div class="flex flex-col gap-2">
      <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.gradientColor') }}</label>
      <div class="flex items-center gap-3">
        <input
          type="color"
          :value="dialogGradientColor"
          class="w-10 h-10 rounded-md border border-white/10 bg-transparent cursor-pointer"
          @input="(e) => (dialogGradientColor = (e.target as HTMLInputElement).value)"
        />
        <input
          type="text"
          :value="dialogGradientColor"
          class="flex-1 px-3 py-2 rounded-lg border border-white/10 bg-white/5 text-sm text-white/80 font-mono outline-none focus:border-brand/50"
          @input="(e) => (dialogGradientColor = (e.target as HTMLInputElement).value)"
        />
        <button
          class="px-3 py-1.5 rounded-md text-xs font-bold border border-white/10 bg-white/5 text-white/60 hover:bg-white/10"
          :title="$t('settings.background.dialog.resetGradientTitle')"
          @click="resetDialogGradientColor"
        >
          {{ $t('settings.background.dialog.resetDefault') }}
        </button>
      </div>
    </div>

    <!-- 文字颜色 -->
    <div class="flex flex-col gap-2">
      <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.textColor') }}</label>
      <div class="flex items-center gap-3">
        <input
          type="color"
          :value="dialogTextColor"
          class="w-10 h-10 rounded-md border border-white/10 bg-transparent cursor-pointer"
          @input="(e) => (dialogTextColor = (e.target as HTMLInputElement).value)"
        />
        <input
          type="text"
          :value="dialogTextColor"
          class="flex-1 px-3 py-2 rounded-lg border border-white/10 bg-white/5 text-sm text-white/80 font-mono outline-none focus:border-brand/50"
          @input="(e) => (dialogTextColor = (e.target as HTMLInputElement).value)"
        />
        <button
          class="px-3 py-1.5 rounded-md text-xs font-bold border border-white/10 bg-white/5 text-white/60 hover:bg-white/10"
          :title="$t('settings.background.dialog.resetTextTitle')"
          @click="resetDialogTextColor"
        >
          {{ $t('settings.background.dialog.resetDefault') }}
        </button>
      </div>
    </div>

    <!-- 实时预览 -->
    <div class="flex flex-col gap-2 mt-2">
      <label class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.preview') }}</label>
      <div
        class="p-4 transition-all"
        :style="{
          backgroundImage: dialogBackgroundImage ? `url(${dialogBackgroundImage})` : 'none',
          backgroundColor: dialogBackgroundImage ? 'transparent' : dialogGradientWithAlpha,
          backgroundSize: 'cover',
          backgroundPosition: 'center',
          backdropFilter: dialogBackgroundImage ? `blur(${dialogBlur}px)` : 'none',
          borderRadius: dialogBorderRadius + 'px',
          color: dialogTextColor,
        }"
      >
        <div
          class="text-base font-bold"
          :style="{ color: dialogTextColor }"
        >
          {{ $t('settings.background.dialog.previewName') }}
        </div>
        <div
          class="text-sm opacity-70 mt-1"
          :style="{ color: dialogTextColor }"
        >
          {{ $t('settings.background.dialog.previewPlaceholder') }}
        </div>
      </div>
    </div>

    <!-- 全部重置 -->
    <div class="mt-2">
      <button
        class="px-4 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-red-500/20 border-red-500/30 text-red-300 hover:bg-red-500/30"
        @click="resetAllDialogAppearance"
      >
        <RotateCcw :size="14" class="inline-block mr-1" />
        {{ $t('settings.background.dialog.resetAll') }}
      </button>
    </div>

    <!-- 交互行为 -->
    <div class="mt-4 pt-4 border-t border-white/10 flex flex-col gap-3">
      <div class="text-white/70 text-sm font-medium">{{ $t('settings.background.dialog.interaction') }}</div>
      <div class="flex items-center justify-between">
        <span class="text-white/60 text-sm">{{ $t('settings.background.dialog.scrollHistory') }}</span>
        <Toggle :model-value="dialogScrollHistoryEnabled" @update:model-value="(v: boolean) => (dialogScrollHistoryEnabled = v)" />
      </div>
      <div class="flex items-center justify-between">
        <span class="text-white/60 text-sm">{{ $t('settings.background.dialog.spacebarHide') }}</span>
        <Toggle :model-value="dialogSpacebarHideEnabled" @update:model-value="(v: boolean) => (dialogSpacebarHideEnabled = v)" />
      </div>
      <div class="flex items-center justify-between">
        <span class="text-white/60 text-sm">{{ $t('settings.background.dialog.autoHideOnThink') }}</span>
        <Toggle :model-value="dialogAutoHideOnThinkEnabled" @update:model-value="(v: boolean) => (dialogAutoHideOnThinkEnabled = v)" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Slider, Toggle } from '../../base'
import { useSettingsStore } from '@/stores/modules/settings'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { Upload, RotateCcw } from 'lucide-vue-next'
import { hexToRgba } from '@/utils/color'

const settingsStore = useSettingsStore()
const dialogStore = useDialogStore()
const { t } = useI18n()

const dialogBgInput = ref<HTMLInputElement | null>(null)

const dialogBackgroundImage = computed(() => settingsStore.dialogBackgroundImage)
const dialogOpacity = computed({
  get: () => settingsStore.dialogOpacity,
  set: (v: number) => settingsStore.setDialogOpacity(v),
})
const dialogBlur = computed({
  get: () => settingsStore.dialogBlur,
  set: (v: number) => settingsStore.setDialogBlur(v),
})
const dialogBorderRadius = computed({
  get: () => settingsStore.dialogBorderRadius,
  set: (v: number) => settingsStore.setDialogBorderRadius(v),
})
const dialogGradientColor = computed({
  get: () => settingsStore.dialogGradientColor,
  set: (v: string) => settingsStore.setDialogGradientColor(v),
})
const dialogTextColor = computed({
  get: () => settingsStore.dialogTextColor,
  set: (v: string) => settingsStore.setDialogTextColor(v),
})
const dialogScrollHistoryEnabled = computed({
  get: () => settingsStore.dialogScrollHistoryEnabled,
  set: (v: boolean) => settingsStore.setDialogScrollHistoryEnabled(v),
})
const dialogSpacebarHideEnabled = computed({
  get: () => settingsStore.dialogSpacebarHideEnabled,
  set: (v: boolean) => settingsStore.setDialogSpacebarHideEnabled(v),
})
const dialogAutoHideOnThinkEnabled = computed({
  get: () => settingsStore.dialogAutoHideOnThinkEnabled,
  set: (v: boolean) => settingsStore.setDialogAutoHideOnThinkEnabled(v),
})

const dialogOpacityPercent = computed({
  get: () => Math.round(dialogOpacity.value * 100),
  set: (v: number) => settingsStore.setDialogOpacity(v / 100),
})

const dialogGradientWithAlpha = computed(() => {
  const hex = dialogGradientColor.value || '#000e27'
  const alpha = dialogOpacity.value
  return hexToRgba(hex, alpha)
})

function triggerDialogBgUpload(): void {
  dialogBgInput.value?.click()
}

async function handleDialogBgUpload(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  if (file.size > 2 * 1024 * 1024) {
    await dialogStore.alert(t('settings.background.dialog.imageTooLarge'))
    if (target) target.value = ''
    return
  }
  const allowedExts = ['.jpg', '.jpeg', '.png', '.webp', '.bmp']
  const fileName = file.name.toLowerCase()
  if (!allowedExts.some((ext) => fileName.endsWith(ext))) {
    await dialogStore.alert(t('settings.background.dialog.unsupportedFormat') + allowedExts.join(', '))
    if (target) target.value = ''
    return
  }

  try {
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(reader.result as string)
      reader.onerror = () => reject(new Error('read failed'))
      reader.readAsDataURL(file)
    })
    settingsStore.setDialogBackgroundImage(dataUrl)
  } catch (e) {
    console.error('读取图片失败', e)
    await dialogStore.alert(t('settings.background.dialog.readFailed'))
  }
  if (target) target.value = ''
}

function clearDialogBackgroundImage(): void {
  settingsStore.setDialogBackgroundImage('')
}

function resetDialogGradientColor(): void {
  settingsStore.setDialogGradientColor('#000e27')
}

function resetDialogTextColor(): void {
  settingsStore.setDialogTextColor('#ffffff')
}

function resetAllDialogAppearance(): void {
  settingsStore.resetDialogAppearance()
}
</script>
