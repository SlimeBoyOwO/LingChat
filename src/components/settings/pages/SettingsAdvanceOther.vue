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
        💡 部分设置需要重启，具体以当前页面说明为准。
      </div>

      <div
        v-for="(categoryData, categoryName) in configData"
        :key="categoryName"
        class="flex flex-col gap-1 w-full"
      >
        <span
          class="text-base font-bold px-3.75 py-2.5 block rounded-lg mb-1 text-brand bg-white/10 backdrop-blur-xl backdrop-saturate-150 border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)]"
        >{{ categoryName }}</span>
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
          {{ subcategoryName }}
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
        返回设置列表
      </button>
      <div v-if="selectedSubcategory" class="w-full active">
        <div class="pt-2.5 overflow-auto">
          <header class="pb-4 mb-6 border-b border-brand">
            <h2 class="m-0 text-2xl text-brand font-semibold">
              {{ activeSelection.subcategory }}
            </h2>
            <p class="mt-2 text-base">
              {{
                selectedSubcategory.description ||
                `修改 ${activeSelection.subcategory} 的相关配置`
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
                :setting="setting"
                :readonly="isWindowDimensionLocked(setting)"
                :helper-text="windowDimensionHelperText(setting)"
                @update:value="(value) => (setting.value = value)"
              />
            </div>

            <!-- 保存操作区域 -->
            <div class="flex flex-col items-start gap-2">
              <button
                type="submit"
                :disabled="isLoading"
                :aria-busy="isLoading"
                class="min-w-30 rounded-lg bg-brand px-5 py-2.5 text-sm font-semibold text-slate-950 shadow-sm transition-colors duration-200 hover:bg-[#8be3ff] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white focus-visible:ring-offset-2 focus-visible:ring-offset-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {{ isLoading ? '保存中…' : '保存' }}
              </button>
              <p
                v-if="saveStatus.message"
                role="status"
                aria-live="polite"
                aria-atomic="true"
                :class="saveStatus.colorClass"
                class="max-w-125 rounded-lg border px-3 py-2 text-sm leading-5 whitespace-normal wrap-break-word"
              >
                {{ saveStatus.message }}
              </p>
            </div>
          </form>
        </div>
      </div>
      <div v-else-if="!isLoading && !Object.keys(configData).length" class="w-full active">
        <div class="advanced-settings-container">
          <header>
            <h2 class="adv-title">加载失败</h2>
            <p class="adv-description">无法加载配置或配置为空。</p>
          </header>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, reactive, watch, nextTick } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import SettingItem from '@/components/base/items/SettingItem.vue'
import { getEnvConfigSettings, saveEnvConfigSettings } from '@/api/services/config'

// --- 响应式状态定义 ---
const uiStore = useUIStore()
const narrowViewLevel = ref<'menu' | 'content'>('menu')
const isLoading = ref(false)
const configData = ref<Record<string, any>>({})
const activeSelection = reactive({
  category: null as string | null,
  subcategory: null as string | null,
})
const saveStatus = reactive({
  message: '',
  colorClass: 'border-emerald-400/60 bg-emerald-950/90 text-emerald-100',
})

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

// 分辨率预设切换时自动同步宽高输入框
const WINDOW_RESOLUTION_PRESET_KEY = 'ui.window_resolution_preset'
const WINDOW_WIDTH_KEY = 'ui.window_width'
const WINDOW_HEIGHT_KEY = 'ui.window_height'

const presetSizeMap: Record<string, { width: string; height: string }> = {
  default: { width: '1500', height: '800' },
  '1920x1080': { width: '1920', height: '1080' },
  '2560x1440': { width: '2560', height: '1440' },
  '1280x720': { width: '1280', height: '720' },
}

const windowResolutionPreset = computed<string | undefined>(() =>
  selectedSubcategory.value?.settings?.find(
    (setting: any) => setting.key === WINDOW_RESOLUTION_PRESET_KEY,
  )?.value,
)

const isWindowDimension = (setting: { key: string }) =>
  setting.key === WINDOW_WIDTH_KEY || setting.key === WINDOW_HEIGHT_KEY

const isWindowDimensionLocked = (setting: { key: string }) =>
  isWindowDimension(setting) && windowResolutionPreset.value !== 'custom'

const windowDimensionHelperText = (setting: { key: string }): string => {
  if (!isWindowDimensionLocked(setting)) return ''
  if (windowResolutionPreset.value === 'fit') {
    return '保存时会根据当前显示器的可用工作区计算尺寸；这里显示的是上次保存的尺寸。'
  }
  return '宽高由当前分辨率预设决定；如需手动输入，请选择“自定义”。'
}

watch(
  () =>
    selectedSubcategory.value?.settings?.find(
      (s: any) => s.key === WINDOW_RESOLUTION_PRESET_KEY,
    )?.value,
  (newPreset, oldPreset) => {
    if (!newPreset || newPreset === oldPreset) return
    if (newPreset === 'custom') return
    const size = presetSizeMap[newPreset]
    if (!size || !selectedSubcategory.value) return
    const settings = selectedSubcategory.value.settings
    const widthSetting = settings.find((s: any) => s.key === WINDOW_WIDTH_KEY)
    const heightSetting = settings.find((s: any) => s.key === WINDOW_HEIGHT_KEY)
    if (widthSetting) widthSetting.value = size.width
    if (heightSetting) heightSetting.value = size.height
  },
)

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
  if (!selectedSubcategory.value || isLoading.value) return

  const formData: Record<string, string> = {}
  selectedSubcategory.value.settings.forEach((setting: { key: string; value: string }) => {
    formData[setting.key] = setting.value
  })

  isLoading.value = true
  saveStatus.message = '正在保存…'
  saveStatus.colorClass = 'border-sky-400/60 bg-sky-950/90 text-sky-100'

  try {
    const saveResult = await saveEnvConfigSettings(formData)
    saveStatus.message = saveResult.message
    saveStatus.colorClass = 'border-emerald-400/60 bg-emerald-950/90 text-emerald-100'

    await loadConfig(false)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error || '未知错误')
    saveStatus.message = `错误: ${message}`
    saveStatus.colorClass = 'border-red-400/60 bg-red-950/90 text-red-100'
  } finally {
    isLoading.value = false
    setTimeout(() => {
      saveStatus.message = ''
    }, 5000)
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
    saveStatus.message = `加载配置失败: ${error.message}`
    saveStatus.colorClass = 'border-red-400/60 bg-red-950/90 text-red-100'
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
