<template>
  <div class="p-6 overflow-y-auto h-full">
    <!-- 标题栏（与 SettingsAdvanceOther 一致：brand 色 + 下边框分隔） -->
    <header class="pb-4 mb-6 border-b border-brand">
      <h2 class="text-2xl text-brand font-semibold">{{ $t('advance.embedding.title') }}</h2>
      <p class="mt-2 text-sm text-white/70">{{ $t('advance.embedding.desc') }}</p>
    </header>

    <!-- 记忆嵌入配置表单（来自后端配置树「记忆嵌入」分类） -->
    <section v-if="embeddingSettings.length" class="mb-6">
      <form @submit.prevent="saveSettings">
        <div
          v-for="setting in embeddingSettings"
          :key="setting.key"
          class="mb-6"
        >
          <SettingItem
            :setting="localizedSetting(setting)"
            @update:value="(value) => (setting.value = value)"
          />
        </div>

        <!-- 保存操作区域（与 SettingsAdvanceOther 保持一致） -->
        <div
          class="inline-flex flex-col gap-2 px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3] min-w-30"
          @click="saveSettings"
        >
          <button
            class="bg-transparent border-none text-white cursor-pointer p-0 m-0 w-full h-full"
          >
            {{ $t('settings.advanceOther.saveButton') }}
          </button>
          <p
            :class="saveStatus.colorClass"
            class="text-xs whitespace-normal wrap-break-word max-w-75"
          >
            {{ saveStatus.message }}
          </p>
        </div>
      </form>
    </section>

    <!-- 记忆嵌入运行状态 -->
    <section class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4">
      <h3 class="mb-1 text-base font-semibold text-white">
        {{ $t('settings.advanceOther.embeddingStatus.title') }}
      </h3>
      <p class="mb-3 text-sm leading-6 text-white/65">
        {{ $t('settings.advanceOther.embeddingStatus.desc') }}
      </p>

      <div class="space-y-1.5 text-sm">
        <div class="flex items-center gap-2">
          <span class="text-white/70">{{ $t('settings.advanceOther.embeddingStatus.enabled') }}</span>
          <span :class="getEnabledClass(embeddingStatus?.enabled)">
            {{ getEnabledText(embeddingStatus?.enabled) }}
          </span>
          <span
            v-if="!embeddingStatus"
            class="text-white/40 text-xs"
          >{{ $t('settings.advanceOther.embeddingStatus.loading') }}</span>
        </div>
        <div
          v-if="embeddingStatus"
          class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-1.5 text-sm"
        >
          <div class="flex items-center gap-2">
            <span class="text-white/70">{{ $t('settings.advanceOther.embeddingStatus.ready') }}</span>
            <span :class="embeddingStatus.ready ? 'text-green-400' : 'text-yellow-400'">
              {{ yesNoLabel(embeddingStatus.ready) }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-white/70">{{ $t('settings.advanceOther.embeddingStatus.configured') }}</span>
            <span :class="embeddingStatus.configured ? 'text-green-400' : 'text-red-400'">
              {{ yesNoLabel(embeddingStatus.configured) }}
            </span>
          </div>
          <div v-if="embeddingStatus.dim" class="flex items-center gap-2">
            <span class="text-white/70">{{ $t('settings.advanceOther.embeddingStatus.dim') }}</span>
            <span class="text-white">{{ embeddingStatus.dim }}</span>
          </div>
          <div v-if="embeddingStatus.model" class="flex items-center gap-2">
            <span class="text-white/70">{{ $t('settings.advanceOther.embeddingStatus.model') }}</span>
            <span class="text-white break-all">{{ embeddingStatus.model }}</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-white/70">{{ $t('settings.advanceOther.embeddingStatus.indexLen') }}</span>
            <span class="text-white">{{ embeddingStatus.indexLen }}</span>
          </div>
        </div>
        <p
          v-if="embeddingStatus && !embeddingStatus.ready && embeddingStatus.error"
          class="mt-2 text-sm text-red-400 break-all"
        >
          {{ embeddingStatus.error }}
        </p>
      </div>

      <Button type="big" :disabled="isLoadingEmbedding" class="mt-3" @click="loadEmbeddingStatus">
        <RefreshCw :size="18" :class="{ 'animate-spin': isLoadingEmbedding }" />
        {{ $t('settings.advanceOther.embeddingStatus.refresh') }}
      </Button>
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingItem from '@/components/base/items/SettingItem.vue'
import { Button } from '@/components/base'
import { getEmbeddingStatus, getEnvConfigSettings, saveEnvConfigSettings, type EmbeddingStatus } from '@/api/services/config'
import { RefreshCw } from 'lucide-vue-next'

// 后端配置树（config/tree.rs）中「记忆嵌入」分类名
const EMBEDDING_CATEGORY = '记忆嵌入'

const { t, te } = useI18n()

// 后端配置树的描述均为中文，按键查 i18n 词条做界面本地化；查不到时回退后端原文
const localizedSetting = (setting: any) => ({
  ...setting,
  description: te(`settings.advanceOther.fields.${setting.key}`)
    ? t(`settings.advanceOther.fields.${setting.key}`)
    : setting.description,
})

// --- 配置表单 ---
const embeddingSettings = ref<any[]>([])
const saveStatus = reactive({
  message: '',
  colorClass: 'text-green-500',
})

// --- 运行状态 ---
const isLoadingEmbedding = ref(false)
const embeddingStatus = ref<EmbeddingStatus | null>(null)

const getEnabledClass = (enabled?: boolean) =>
  enabled ? 'text-green-400' : 'text-gray-400'
const getEnabledText = (enabled?: boolean) =>
  enabled
    ? t('settings.advanceOther.embeddingStatus.labelOn')
    : t('settings.advanceOther.embeddingStatus.labelOff')
const yesNoLabel = (value: boolean) =>
  value
    ? t('settings.advanceOther.embeddingStatus.labelYes')
    : t('settings.advanceOther.embeddingStatus.labelNo')

const loadEmbeddingStatus = async () => {
  isLoadingEmbedding.value = true
  try {
    embeddingStatus.value = await getEmbeddingStatus()
  } catch (error) {
    console.error('加载记忆嵌入状态失败:', error)
    embeddingStatus.value = null
  } finally {
    isLoadingEmbedding.value = false
  }
}

const loadConfig = async () => {
  try {
    const configData = await getEnvConfigSettings()
    const category = configData[EMBEDDING_CATEGORY]
    const subcategories = category?.subcategories || {}
    const firstSubcategory = Object.keys(subcategories)[0]
    if (firstSubcategory) {
      embeddingSettings.value = subcategories[firstSubcategory].settings || []
    }
  } catch (error: any) {
    console.error(error)
    saveStatus.message = t('settings.advanceOther.msg.loadConfigFailed', { error: error.message })
    saveStatus.colorClass = 'text-red-500'
  }
}

const saveSettings = async () => {
  if (!embeddingSettings.value.length) return

  const formData: Record<string, string> = {}
  embeddingSettings.value.forEach((setting: { key: string; value: string }) => {
    formData[setting.key] = setting.value
  })

  saveStatus.message = ''
  try {
    saveStatus.message = (await saveEnvConfigSettings(formData)).message
    saveStatus.colorClass = 'text-green-500'
  } catch (error: any) {
    saveStatus.message = t('settings.advanceOther.msg.error', { error: error.message })
    saveStatus.colorClass = 'text-red-500'
  } finally {
    setTimeout(() => {
      saveStatus.message = ''
    }, 5000)
  }
}

onMounted(async () => {
  await loadConfig()
  await loadEmbeddingStatus()
})
</script>