<template>
  <MenuPage>
    <div class="flex-1 h-[85vh] w-full bg-white/10 p-0 md:p-4 rounded-lg overflow-hidden flex flex-col">
      <!-- 顶部 Tab 切换栏：左 / 中 / 右 -->
      <div class="flex items-center justify-between mb-5 select-none shrink-0">
        <button
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
          :class="advanceTab === 'menu'
            ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
            : 'text-white/60 hover:text-white/80'"
          @click="advanceTab = 'menu'"
        >
          {{ $t('advance.tabs.menu') }}
        </button>
        <button
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
          :class="advanceTab === 'llm'
            ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
            : 'text-white/60 hover:text-white/80'"
          @click="advanceTab = 'llm'"
        >
          {{ $t('advance.tabs.llm') }}
        </button>
        <button
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
          :class="advanceTab === 'tts'
            ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
            : 'text-white/60 hover:text-white/80'"
          @click="advanceTab = 'tts'"
        >
          {{ $t('advance.tabs.tts') }}
        </button>
        <button
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
          :class="advanceTab === 'tools'
            ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
            : 'text-white/60 hover:text-white/80'"
          @click="advanceTab = 'tools'"
        >
          {{ $t('advance.tabs.tools') }}
        </button>
        <button
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
          :class="advanceTab === 'other'
            ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
            : 'text-white/60 hover:text-white/80'"
          @click="advanceTab = 'other'"
        >
          {{ $t('advance.tabs.other') }}
        </button>
      </div>

      <!-- ====== 主菜单 ====== -->
      <div v-if="advanceTab === 'menu'" class="flex-1 min-h-0 overflow-y-auto">
        <SettingsAdvanceMenu @navigate="advanceTab = $event" />
      </div>

      <!-- ====== 大模型管理 ====== -->
      <div v-else-if="advanceTab === 'llm'" class="flex-1 min-h-0">
        <SettingsLlmProviders />
      </div>

      <!-- ====== 本地 TTS ====== -->
      <div v-else-if="advanceTab === 'tts'" class="flex-1 min-h-0">
        <SettingsTts />
      </div>

      <!-- ====== 工具配置 ====== -->
      <div v-else-if="advanceTab === 'tools'" class="flex-1 min-h-0">
        <SettingsTools />
      </div>

      <!-- ====== 其他高级设置 ====== -->
      <div v-else class="flex-1 min-h-0">
        <SettingsAdvanceOther
          ref="advanceOtherRef"
          @remove-more-menu-from-b="emit('remove-more-menu-from-b')"
        />
      </div>
    </div>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { MenuPage } from '../../ui'
import SettingsLlmProviders from './SettingsLlmProviders.vue'
import SettingsAdvanceMenu from './SettingsAdvanceMenu.vue'
import SettingsTts from './SettingsTts.vue'
import SettingsTools from './SettingsTools.vue'
import SettingsAdvanceOther from './SettingsAdvanceOther.vue'
import { useUIStore } from '@/stores/modules/ui/ui'

const uiStore = useUIStore()

// 子标签状态放在 ui store，供「日程 → 工具调用」等入口直接跳转定位
const advanceTab = computed({
  get: () => uiStore.advanceTab,
  set: (tab: string) => {
    uiStore.advanceTab = tab
  },
})

const advanceOtherRef = ref<InstanceType<typeof SettingsAdvanceOther> | null>(null)

const emit = defineEmits<{
  'remove-more-menu-from-b': []
}>()

const addMoreMenu = () => {
  advanceOtherRef.value?.addMoreMenu()
}

defineExpose({
  addMoreMenu,
})
</script>
