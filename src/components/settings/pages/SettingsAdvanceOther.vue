<template>
  <div class="flex flex-col md:grid md:grid-cols-[min(30%,280px)_1fr] h-full min-h-0">
    <!-- 导航菜单：宽屏始终可见；窄屏仅在浏览菜单层级时可见 -->
    <nav
      ref="navContainerRef"
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'menu'"
      @click="() => removeMoreMenu()"
      class="transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)] flex flex-col justify-start gap-6.25 overflow-y-auto relative border-b md:border-b-0 md:border-r border-brand md:moreMenu:left-0"
      :class="[
        'md:left-0',
        'translate-y-0',
        'moreMenu:translate-y-0',
      ]"
    >
      <!-- 滑动指示器 -->
      <div
        ref="indicatorRef"
        class="absolute left-2 w-[calc(100%-40px)] bg-brand rounded-lg z-0 transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)]"
      ></div>

      <div
        class="flex items-center gap-1 mt-2 text-sm px-5"
        style="color: white; -webkit-text-stroke: 1px black; paint-order: stroke fill"
      >
        {{ $t('settings.advanceOther.restartHint') }}
      </div>

      <div
        v-for="(categoryData, categoryName) in configData"
        :key="categoryName"
        class="flex flex-col gap-1 w-full"
      >
        <span
          class="text-base font-bold px-3.75 py-2.5 block rounded-lg mb-1 text-brand bg-white/10 backdrop-blur-xl backdrop-saturate-150 border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)]"
        >{{ catLabel(categoryName) }}</span>
        <a
          v-for="(, subcategoryName) in categoryData.subcategories"
          :key="subcategoryName"
          href="#"
          class="block px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
          :class="{
            active: isActive(categoryName, subcategoryName.toString()),
          }"
          @click.prevent="selectSubcategory(categoryName, subcategoryName.toString())"
        >
          {{ subLabel(subcategoryName) }}
        </a>
      </div>
    </nav>

    <!-- 设置内容区域：宽屏始终可见；窄屏仅在浏览内容层级时可见 -->
    <main
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'content'"
      class="flex justify-center h-full overflow-auto relative px-10 py-10 md:px-10 md:py-0"
      :class="[
        'translate-y-0',
        'moreMenu:translate-y-0',
      ]"
    >
      <!-- 窄屏返回按钮 -->
      <button
        v-if="uiStore.isNarrowScreen"
        class="absolute top-0 left-4 flex items-center gap-1.5 text-sm text-white/70 hover:text-white transition-colors py-1 px-2 rounded-lg hover:bg-white/10"
        @click="narrowViewLevel = 'menu'"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/></svg>
        {{ $t('settings.advanceOther.backToList') }}
      </button>
      <div v-if="selectedSubcategory" class="w-full active">
        <div class="pt-2.5 overflow-auto">
          <header class="pb-4 mb-6 border-b border-brand">
            <h2 class="m-0 text-2xl text-brand font-semibold">
              {{ subLabel(activeSelection.subcategory ?? '') }}
            </h2>
            <p class="mt-2 text-base">
              {{
                subDesc(activeSelection.subcategory ?? '', selectedSubcategory.description) ||
                $t('settings.advanceOther.subcategoryDesc', { name: subLabel(activeSelection.subcategory ?? '') })
              }}
            </p>
          </header>

          <form @submit.prevent="saveSettings">
            <div
              v-for="setting in selectedSubcategory.settings"
              :key="setting.key"
              class="mb-6"
            >
              <SettingItem
                :setting="localizedSetting(setting)"
                @update:value="(value) => (setting.value = value)"
              />
            </div>
          </form>

          <section
            v-if="activeSelection.category === 'TTS 配置'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">{{ $t('settings.advanceOther.ttsControl.title') }}</h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t('settings.advanceOther.ttsControl.desc') }}
            </p>
            <Button type="big" :disabled="isReconnectingTts" @click="forceReconnectTts">
              <RefreshCw :size="18" :class="{ 'animate-spin': isReconnectingTts }" />
              {{ isReconnectingTts ? $t('settings.advanceOther.ttsControl.reconnecting') : $t('settings.advanceOther.ttsControl.forceReconnect') }}
            </Button>
            <p
              v-if="reconnectStatus.message"
              class="mt-2 text-sm"
              :class="reconnectStatus.colorClass"
            >
              {{ reconnectStatus.message }}
            </p>
          </section>

          <!-- 记忆嵌入运行状态 -->
          <section
            v-if="activeSelection.category === '记忆嵌入'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
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

          <!-- 独立语义记忆运行状态 -->
          <section
            v-if="activeSelection.category === '语义记忆'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t('settings.advanceOther.semanticMemoryStatus.title') }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t('settings.advanceOther.semanticMemoryStatus.desc') }}
            </p>

            <div class="space-y-1.5 text-sm">
              <div class="flex items-center gap-2">
                <span class="text-white/70">{{ $t('settings.advanceOther.semanticMemoryStatus.enabled') }}</span>
                <span :class="getEnabledClass(semanticMemoryStatus?.enabled)">
                  {{ getEnabledText(semanticMemoryStatus?.enabled) }}
                </span>
                <span
                  v-if="!semanticMemoryStatus"
                  class="text-white/40 text-xs"
                >{{ $t('settings.advanceOther.semanticMemoryStatus.loading') }}</span>
              </div>
              <div
                v-if="semanticMemoryStatus"
                class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-1.5 text-sm"
              >
                <div class="flex items-center gap-2">
                  <span class="text-white/70">{{ $t('settings.advanceOther.semanticMemoryStatus.opened') }}</span>
                  <span :class="semanticMemoryStatus.opened ? 'text-green-400' : 'text-yellow-400'">
                    {{ yesNoLabel(semanticMemoryStatus.opened) }}
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  <span class="text-white/70">{{ $t('settings.advanceOther.semanticMemoryStatus.embeddingReady') }}</span>
                  <span :class="semanticMemoryStatus.embeddingReady ? 'text-green-400' : 'text-red-400'">
                    {{ yesNoLabel(semanticMemoryStatus.embeddingReady) }}
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  <span class="text-white/70">{{ $t('settings.advanceOther.semanticMemoryStatus.count') }}</span>
                  <span class="text-white">{{ semanticMemoryStatus.count }}</span>
                </div>
                <div v-if="semanticMemoryStatus.dbPath" class="flex items-center gap-2 col-span-full">
                  <span class="text-white/70">{{ $t('settings.advanceOther.semanticMemoryStatus.dbPath') }}</span>
                  <span class="text-white break-all">{{ semanticMemoryStatus.dbPath }}</span>
                </div>
              </div>
              <p
                v-if="semanticMemoryStatus && !semanticMemoryStatus.embeddingReady"
                class="mt-2 text-sm text-yellow-400"
              >
                {{ $t('settings.advanceOther.semanticMemoryStatus.needEmbedding') }}
              </p>
              <p
                v-if="semanticMemoryStatus && semanticMemoryStatus.error"
                class="mt-2 text-sm text-red-400 break-all"
              >
                {{ semanticMemoryStatus.error }}
              </p>
            </div>

            <Button type="big" :disabled="isLoadingSemanticMemory" class="mt-3" @click="loadSemanticMemoryStatus">
              <RefreshCw :size="18" :class="{ 'animate-spin': isLoadingSemanticMemory }" />
              {{ $t('settings.advanceOther.semanticMemoryStatus.refresh') }}
            </Button>
          </section>

          <!-- 保存操作区域 -->
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
        </div>
      </div>
      <div v-else-if="!isLoading && !Object.keys(configData).length" class="w-full active">
        <div class="advanced-settings-container">
          <header>
            <h2 class="adv-title">{{ $t('settings.advanceOther.loadFailed') }}</h2>
            <p class="adv-description">{{ $t('settings.advanceOther.loadFailedDesc') }}</p>
          </header>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, reactive, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUIStore } from '@/stores/modules/ui/ui'
import SettingItem from '@/components/base/items/SettingItem.vue'
import { Button } from '@/components/base'
import { getEnvConfigSettings, saveEnvConfigSettings, getEmbeddingStatus, type EmbeddingStatus, getSemanticMemoryStatus, type SemanticMemoryStatus } from '@/api/services/config'
import { reactivateTTS } from '@/api/services/game-info'
import { switchLlm } from '@/api/services/llm-providers'
import { RefreshCw } from 'lucide-vue-next'

// --- 响应式状态定义 ---
const uiStore = useUIStore()
const { t, te } = useI18n()

// 后端配置树的分类/子类/设置项描述均为中文（config/tree.rs），
// 这里按名称/键查 i18n 词条做界面日文化；查不到时回退后端原文。
const catLabel = (name: string) =>
  te(`settings.advanceOther.categories.${name}`) ? t(`settings.advanceOther.categories.${name}`) : name
const subLabel = (name: string) =>
  te(`settings.advanceOther.subcategories.${name}`)
    ? t(`settings.advanceOther.subcategories.${name}`)
    : name
const subDesc = (name: string, fallback: string) =>
  te(`settings.advanceOther.subcategoryDescs.${name}`)
    ? t(`settings.advanceOther.subcategoryDescs.${name}`)
    : fallback
const localizedSetting = (setting: any) => ({
  ...setting,
  description: te(`settings.advanceOther.fields.${setting.key}`)
    ? t(`settings.advanceOther.fields.${setting.key}`)
    : setting.description,
})
const narrowViewLevel = ref<'menu' | 'content'>('menu')
const isLoading = ref(false)
const configData = ref<Record<string, any>>({})
const activeSelection = reactive({
  category: null as string | null,
  subcategory: null as string | null,
})
const saveStatus = reactive({
  message: '',
  colorClass: 'text-green-500',
})
const isReconnectingTts = ref(false)
const reconnectStatus = reactive({
  message: '',
  colorClass: 'text-green-400',
})
let reconnectStatusTimer: ReturnType<typeof setTimeout> | null = null

// --- 记忆嵌入状态 ---
const isLoadingEmbedding = ref(false)
const embeddingStatus = ref<EmbeddingStatus | null>(null)

// --- 独立语义记忆状态 ---
const isLoadingSemanticMemory = ref(false)
const semanticMemoryStatus = ref<SemanticMemoryStatus | null>(null)

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

const loadSemanticMemoryStatus = async () => {
  isLoadingSemanticMemory.value = true
  try {
    semanticMemoryStatus.value = await getSemanticMemoryStatus()
  } catch (error) {
    console.error('加载语义记忆状态失败:', error)
    semanticMemoryStatus.value = null
  } finally {
    isLoadingSemanticMemory.value = false
  }
}

const emit = defineEmits<{
  'remove-more-menu-from-b': []
}>()

// --- Refs for DOM elements ---
const navContainerRef = ref<HTMLElement | null>(null)
const indicatorRef = ref<HTMLElement | null>(null)

// --- 计算属性 ---
const selectedSubcategory = computed(() => {
  if (activeSelection.category && activeSelection.subcategory) {
    return configData.value[activeSelection.category]?.subcategories[activeSelection.subcategory]
  }
  return null
})

// --- 方法定义 ---

const isActive = (category: string, subcategory: string) => {
  return activeSelection.category === category && activeSelection.subcategory === subcategory
}

const selectSubcategory = (category: string, subcategory: string) => {
  activeSelection.category = category
  activeSelection.subcategory = subcategory
  // 窄屏下自动切换到内容视图
  if (uiStore.isNarrowScreen) {
    narrowViewLevel.value = 'content'
  }
}

const saveSettings = async () => {
  if (!selectedSubcategory.value) return

  const formData: Record<string, string> = {}
  selectedSubcategory.value.settings.forEach((setting: { key: string; value: string }) => {
    formData[setting.key] = setting.value
  })

  isLoading.value = true
  saveStatus.message = ''

  try {
    saveStatus.message = (await saveEnvConfigSettings(formData)).message
    if (Object.prototype.hasOwnProperty.call(formData, 'llm.timeout_secs')) {
      await switchLlm()
    }
    saveStatus.colorClass = 'text-green-500'

    await loadConfig(false)
  } catch (error: any) {
    saveStatus.message = t('settings.advanceOther.msg.error', { error: error.message })
    saveStatus.colorClass = 'text-red-500'
  } finally {
    isLoading.value = false
    setTimeout(() => {
      saveStatus.message = ''
    }, 5000)
  }
}

const forceReconnectTts = async () => {
  if (isReconnectingTts.value) return

  isReconnectingTts.value = true
  reconnectStatus.message = t('settings.advanceOther.msg.ttsReactivating')
  reconnectStatus.colorClass = 'text-white/70'
  if (reconnectStatusTimer) {
    clearTimeout(reconnectStatusTimer)
    reconnectStatusTimer = null
  }

  try {
    await reactivateTTS()
    reconnectStatus.message = t('settings.advanceOther.msg.ttsReactivated')
    reconnectStatus.colorClass = 'text-green-400'
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error)
    reconnectStatus.message = t('settings.advanceOther.msg.ttsReconnectFailed', { error: message })
    reconnectStatus.colorClass = 'text-red-400'
  } finally {
    isReconnectingTts.value = false
    reconnectStatusTimer = setTimeout(() => {
      reconnectStatus.message = ''
      reconnectStatusTimer = null
    }, 8000)
  }
}

const loadConfig = async (selectFirst = true) => {
  isLoading.value = true
  try {
    configData.value = await getEnvConfigSettings()

    if (selectFirst && Object.keys(configData.value).length > 0) {
      const firstCategory = Object.keys(configData.value)[0]
      if (firstCategory) {
        const firstSubcategory = Object.keys(
          configData.value[firstCategory]?.subcategories || {},
        )[0]

        if (firstCategory && firstSubcategory) {
          selectSubcategory(firstCategory, firstSubcategory)
        }
      }
    }
  } catch (error: any) {
    console.error(error)
    saveStatus.message = t('settings.advanceOther.msg.loadConfigFailed', { error: error.message })
    saveStatus.colorClass = 'text-red-500'
  } finally {
    isLoading.value = false
  }
}

// --- 导航指示器逻辑 ---
const updateIndicatorPosition = () => {
  if (!navContainerRef.value || !indicatorRef.value) return

  const activeLink = navContainerRef.value.querySelector('.adv-nav-link.active') as HTMLElement

  if (activeLink) {
    const top = activeLink.offsetTop
    const height = activeLink.offsetHeight

    if (top) {
      indicatorRef.value.style.top = `${top}px`
    }
    if (height) {
      indicatorRef.value.style.height = `${height}px`
    }
  }
}

// --- 监听导航容器尺寸变化 ---
const setupNavResizeObserver = () => {
  if (!navContainerRef.value) return

  const resizeObserver = new ResizeObserver(() => {
    updateIndicatorPosition()
  })

  resizeObserver.observe(navContainerRef.value)
}

// 监视 activeSelection 的变化，并在 DOM 更新后移动指示器
watch(
  activeSelection,
  async () => {
    await nextTick()
    updateIndicatorPosition()
    // 进入「记忆嵌入」分类时自动拉取运行状态
    if (activeSelection.category === '记忆嵌入') {
      loadEmbeddingStatus()
    }
    // 进入「语义记忆」分类时自动拉取运行状态
    if (activeSelection.category === '语义记忆') {
      loadSemanticMemoryStatus()
    }
  },
  { deep: true },
)

// --- 生命周期钩子 ---
onMounted(async () => {
  await loadConfig()
  await nextTick()
  updateIndicatorPosition()
  setupNavResizeObserver()
})

onUnmounted(() => {
  if (reconnectStatusTimer) {
    clearTimeout(reconnectStatusTimer)
  }
})

// --- 窄屏菜单控制 ---
const addMoreMenu = () => {
  const btnEl = navContainerRef.value as HTMLElement | null
  if (btnEl) {
    btnEl.classList.add('moreMenu')
  }
}

const removeMoreMenu = () => {
  const btnEl = navContainerRef.value as HTMLElement | null
  if (btnEl) {
    btnEl.classList.remove('moreMenu')
  }
  emit('remove-more-menu-from-b')
}

defineExpose({
  addMoreMenu,
})
</script>
