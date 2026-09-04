<template>
  <MenuPage>
    <div
      class="flex h-[85dvh] w-full flex-1 flex-col overflow-hidden rounded-lg bg-white/10 p-0
        md:p-4"
    >
      <!-- 顶部 Tab 切换栏：左 / 中 / 右 -->
      <div class="mb-5 flex shrink-0 items-center justify-between select-none">
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'menu'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'menu'"
        >
          {{ $t("advance.tabs.menu") }}
        </button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'llm'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'llm'"
        >
          {{ $t("advance.tabs.llm") }}
        </button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'tts'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'tts'"
        >
          {{ $t("advance.tabs.tts") }}
        </button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'asr'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'asr'"
        >
          {{ $t("advance.tabs.asr") }}
        </button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'tools'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'tools'"
        >
          {{ $t("advance.tabs.tools") }}
        </button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'cast'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'cast'"
        >
          {{ $t("advance.tabs.cast") }}
        </button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium transition-all duration-200"
          :class="
            advanceTab === 'other'
              ? 'bg-brand text-white shadow-[0_2px_8px_rgba(0,0,0,0.3)]'
              : 'text-white/60 hover:text-white/80'
          "
          @click="advanceTab = 'other'"
        >
          {{ $t("advance.tabs.other") }}
        </button>
      </div>

      <!-- ====== 主菜单 ====== -->
      <div v-if="advanceTab === 'menu'" class="min-h-0 flex-1 overflow-y-auto">
        <SettingsAdvanceMenu @navigate="advanceTab = $event" />
      </div>

      <!-- ====== 大模型管理 ====== -->
      <div v-else-if="advanceTab === 'llm'" class="min-h-0 flex-1">
        <SettingsLlmProviders />
      </div>

      <!-- ====== 本地 TTS ====== -->
      <div v-else-if="advanceTab === 'tts'" class="min-h-0 flex-1">
        <SettingsTts />
      </div>

      <!-- ====== 语音识别 ====== -->
      <div v-else-if="advanceTab === 'asr'" class="min-h-0 flex-1">
        <SettingsAsr />
      </div>

      <!-- ====== 工具配置 ====== -->
      <div v-else-if="advanceTab === 'tools'" class="min-h-0 flex-1">
        <SettingsTools />
      </div>

      <!-- ====== 投影配置 ====== -->
      <div v-else-if="advanceTab === 'cast'" class="min-h-0 flex-1">
        <SettingsCast />
      </div>

      <!-- ====== 其他高级设置 ====== -->
      <div v-else class="min-h-0 flex-1">
        <SettingsAdvanceOther
          ref="advanceOtherRef"
          @remove-more-menu-from-b="emit('remove-more-menu-from-b')"
        />
      </div>
    </div>
  </MenuPage>
</template>

<script setup lang="ts">
  import { ref, computed } from "vue";
  import { MenuPage } from "../../ui";
  import SettingsLlmProviders from "./SettingsLlmProviders.vue";
  import SettingsAdvanceMenu from "./SettingsAdvanceMenu.vue";
  import SettingsTts from "./SettingsTts.vue";
  import SettingsAsr from "./SettingsAsr.vue";
  import SettingsTools from "./SettingsTools.vue";
  import SettingsAdvanceOther from "./SettingsAdvanceOther.vue";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import SettingsCast from "./SettingsCast.vue";

  const uiStore = useUIStore();

  // 子标签状态放在 ui store，供「日程 → 工具调用」等入口直接跳转定位
  const advanceTab = computed({
    get: () => uiStore.advanceTab,
    set: (tab: string) => {
      uiStore.advanceTab = tab;
    },
  });

  const advanceOtherRef = ref<InstanceType<typeof SettingsAdvanceOther> | null>(null);

  const emit = defineEmits<{
    "remove-more-menu-from-b": [];
  }>();

  const addMoreMenu = () => {
    advanceOtherRef.value?.addMoreMenu();
  };

  defineExpose({
    addMoreMenu,
  });
</script>
