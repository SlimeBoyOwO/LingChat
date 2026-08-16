<template>
  <div class="flex
    w-full
    flex-col
    gap-4">
    <div class="flex
      items-center
      justify-between">
      <p class="text-[0.78rem]
        text-white/50">{{ t('scriptEditor.appearance.intro') }}</p>
    </div>

    <!-- 背景图 -->
    <MenuItem :title="t('scriptEditor.appearance.menuTitle')">
      <template #header>
        <Icon
          icon="background"
          :size="16"
          class="text-brand"
        />
      </template>
      <div class="flex
        flex-col
        gap-3">
        <div class="rounded-lg
          border
          border-white/10
          bg-black/20
          px-3
          py-2.5">
          <div class="mb-1
            text-[0.72rem]
            text-white/45">
            {{ t('scriptEditor.appearance.current') }}
          </div>
          <div class="flex
            items-center
            gap-2
            font-mono
            text-[0.78rem]
            text-white/85">
            <Icon
              icon="background"
              :size="13"
              class="shrink-0
                text-white/40"
            />
            <span class="truncate">{{ bgFileName }}</span>
          </div>
        </div>

        <div class="flex
          gap-2">
          <button
            class="inline-flex
              flex-1
              items-center
              justify-center
              gap-1.5
              rounded-lg
              border
              border-brand/45
              bg-brand/14
              px-3
              py-2
              text-[0.8rem]
              text-brand
              transition-colors
              hover:bg-brand/24"
            @click="pickImage"
          >
            <Icon
              icon="background"
              :size="14"
            />
            {{ t('scriptEditor.appearance.pick') }}
          </button>
          <button
            class="inline-flex
              items-center
              justify-center
              gap-1.5
              rounded-lg
              border
              border-white/10
              bg-white/6
              px-3
              py-2
              text-[0.8rem]
              text-white/70
              transition-colors
              hover:bg-white/[0.12]
              hover:text-white"
            :title="t('scriptEditor.appearance.resetTitle')"
            :disabled="!store.editorBg.path"
            @click="store.resetEditorBg()"
          >
            <Icon
              icon="history"
              :size="14"
            />
            {{ t('scriptEditor.appearance.reset') }}
          </button>
        </div>
        <p class="text-[0.72rem]
          leading-relaxed
          text-white/40">
          {{ t('scriptEditor.appearance.resetDesc') }}{{ t('scriptEditor.appearance.pickHint') }}
        </p>

        <!-- 从已有背景选择 -->
        <div>
          <div class="mb-1.5
            text-[0.72rem]
            text-white/45">
            {{ t('scriptEditor.appearance.fromLibrary') }}
          </div>
          <div class="flex
            flex-col
            gap-1.5">
            <button
              v-for="f in store.globalBgFiles"
              :key="f.path"
              class="flex
                items-center
                gap-2
                rounded-[10px]
                border
                px-3
                py-2
                text-left
                transition-all
                duration-200"
              :class="
                store.editorBg.path === f.path
                  ? `border-brand/40
                    bg-brand/10`
                  : `border-white/10
                    bg-white/6
                    hover:border-brand/40`
              "
              @click="store.setEditorBgPath(f.path)"
            >
              <Icon
                icon="background"
                :size="14"
                class="shrink-0
                  text-white/40"
              />
              <span class="min-w-0
                flex-1
                truncate
                text-[0.8rem]
                text-white/85">{{ f.name }}</span>
              <span class="shrink-0
                text-[0.68rem]
                text-white/35">{{ humanSize(f.size) }}</span>
            </button>
            <p
              v-if="!store.globalBgFiles.length"
              class="text-[0.72rem]
                text-white/35"
            >
              {{ t('scriptEditor.appearance.emptyLibrary') }}
            </p>
          </div>
        </div>
      </div>
    </MenuItem>

    <!-- 视觉效果 -->
    <MenuItem :title="t('scriptEditor.appearance.visual')">
      <template #header>
        <Icon
          icon="sliders"
          :size="16"
          class="text-brand"
        />
      </template>
      <div class="flex
        flex-col
        gap-4">
        <div>
          <label
            class="mb-1
              inline-flex
              items-center
              gap-1.5
              text-[0.84rem]
              font-medium
              text-white/80"
          >
            {{ t('scriptEditor.appearance.blur') }}
          </label>
          <Slider
            :model-value="store.editorBg.blur"
            :min="0"
            :max="24"
            :step="1"
            @change="onBlurChange"
          >
            {{ t('scriptEditor.appearance.clear') }}/{{ t('scriptEditor.appearance.soft') }}
          </Slider>
        </div>

        <div>
          <label
            class="mb-1
              inline-flex
              items-center
              gap-1.5
              text-[0.84rem]
              font-medium
              text-white/80"
          >
            {{ t('scriptEditor.appearance.dim') }}
          </label>
          <Slider
            :model-value="dimPercent"
            :min="0"
            :max="90"
            :step="1"
            @change="onDimChange"
          >
            {{ t('scriptEditor.appearance.bright') }}/{{ t('scriptEditor.appearance.dark') }}
          </Slider>
          <p class="mt-1
            text-[0.72rem]
            leading-relaxed
            text-white/40">
            {{ t('scriptEditor.appearance.dimHint') }}
          </p>
        </div>
      </div>
    </MenuItem>

    <!-- 裁剪弹窗：选图后打开 -->
    <ImageCropModal
      v-if="cropSrc"
      :src-path="cropSrc"
      @confirm="onCropConfirm"
      @cancel="cropSrc = null"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { Icon, Slider } from '@/components/base'
import { MenuItem } from '@/components/ui'
import ImageCropModal from '@/components/script-editor/modals/ImageCropModal.vue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

const { t } = useI18n()
const store = useScriptEditorStore()

const IMAGE_EXT = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif']

/** 当前正在裁剪的源文件路径；非空时显示裁剪弹窗 */
const cropSrc = ref<string | null>(null)

/** 当前背景显示名：自定义时取落盘文件名，默认时说明内置背景 */
const bgFileName = computed(() => {
  const path = store.editorBg.path
  if (!path) return t('scriptEditor.appearance.builtin')
  return path.split(/[\\/]/).pop() || path
})

/** 压暗遮罩：UI 用 0~90 整数显示，store 存 0~1 小数 */
const dimPercent = computed(() => Math.round(store.editorBg.dim * 100))

// 本组件只 emit change，不 emit update:modelValue，这里显式回写 store
const onBlurChange = (v: number) => {
  store.editorBg = { ...store.editorBg, blur: v }
}
const onDimChange = (v: number) => {
  store.editorBg = { ...store.editorBg, dim: v / 100 }
}

const humanSize = (n: number) => {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

/** 选图后进入裁剪弹窗（默认选区铺满整图，不拖框即等于不裁剪） */
const pickImage = async () => {
  const picked = await openDialog({
    multiple: false,
    filters: [{ name: t('scriptEditor.fieldRow.image'), extensions: IMAGE_EXT }],
  })
  if (typeof picked !== 'string' || !picked) return
  cropSrc.value = picked
}

const onCropConfirm = async (dataUrl: string, name: string) => {
  cropSrc.value = null
  await store.uploadEditorBgData(dataUrl, name)
}

onMounted(() => {
  void store.refreshGlobalBgFiles()
})
</script>
