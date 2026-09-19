<template>
  <div class="grid grid-cols-1 gap-5 p-2 md:grid-cols-2 lg:grid-cols-3">
    <!-- 大模型管理 -->
    <div class="h-full cursor-pointer transition-all duration-300" @click="emit('navigate', 'llm')">
      <MenuItem :title="$t('advance.menu.llmTitle')" size="large">
        <template #header>
          <Cpu :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.llmDesc") }}
        </p>
        <Button type="big" icon="advance" :icon_size="18">
          {{ $t("advance.menu.llmButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 本地 TTS -->
    <div class="h-full cursor-pointer transition-all duration-300" @click="emit('navigate', 'tts')">
      <MenuItem :title="$t('advance.menu.ttsTitle')" size="large">
        <template #header>
          <AudioLines :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.ttsDesc") }}
        </p>
        <Button type="big" icon="mic" :icon_size="18"> {{ $t("advance.menu.ttsButton") }} </Button>
      </MenuItem>
    </div>

    <!-- 语音识别 -->
    <div class="h-full cursor-pointer transition-all duration-300" @click="emit('navigate', 'asr')">
      <MenuItem :title="$t('advance.menu.asrTitle')" size="large">
        <template #header>
          <Mic :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.asrDesc") }}
        </p>
        <Button type="big" icon="mic" :icon_size="18">
          {{ $t("advance.menu.asrButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 其他高级设置 -->
    <div
      class="h-full cursor-pointer transition-all duration-300"
      @click="emit('navigate', 'other')"
    >
      <MenuItem :title="$t('advance.menu.otherTitle')" size="large">
        <template #header>
          <SlidersHorizontal :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.otherDesc") }}
        </p>
        <Button type="big" icon="setting" :icon_size="18">
          {{ $t("advance.menu.otherButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 工具配置 -->
    <div
      class="h-full cursor-pointer transition-all duration-300"
      @click="emit('navigate', 'tools')"
    >
      <MenuItem :title="$t('advance.menu.toolsTitle')" size="large">
        <template #header>
          <Wrench :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.toolsDesc") }}
        </p>
        <Button type="big" icon="setting" :icon_size="18">
          {{ $t("advance.menu.toolsButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 投屏设置 -->
    <div
      class="h-full cursor-pointer transition-all duration-300"
      @click="emit('navigate', 'cast')"
    >
      <MenuItem :title="$t('advance.menu.castTitle')" size="large">
        <template #header>
          <Cast :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.castDesc") }}
        </p>
        <Button type="big" icon="advance" :icon_size="18">
          {{ $t("advance.menu.castButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 永久记忆调试 -->
    <div
      class="h-full cursor-pointer transition-all duration-300"
      @click="emit('navigate', 'memory')"
    >
      <MenuItem :title="$t('advance.menu.memoryTitle')" size="large">
        <template #header>
          <Database :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.memoryDesc") }}
        </p>
        <Button type="big" icon="advance" :icon_size="18">
          {{ $t("advance.menu.memoryButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 界面语言 -->
    <div class="h-full transition-all duration-300">
      <MenuItem :title="$t('advance.menu.languageTitle')" size="large">
        <template #header>
          <Languages :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.languageDesc") }}
        </p>
        <select
          :value="locale"
          class="w-full cursor-pointer rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white/80 transition-all duration-200 hover:border-white/30 hover:text-white focus:border-[rgba(121,217,255,0.6)] focus:outline-none"
          @change="setLocale(($event.target as HTMLSelectElement).value as AppLocale)"
        >
          <option
            v-for="opt in SUPPORTED_LOCALES"
            :key="opt.value"
            :value="opt.value"
            class="bg-slate-800 text-white"
          >
            {{ opt.label }}
          </option>
        </select>
      </MenuItem>
    </div>

    <!-- 好感度系统 -->
    <div class="h-full transition-all duration-300">
      <MenuItem :title="$t('advance.menu.affectionTitle')" size="large">
        <template #header>
          <Heart :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.affectionDesc") }}
        </p>
        <Toggle
          :key="affectionMasterEpoch"
          :checked="affectionMasterEnabled"
          @change="onAffectionMasterToggle"
        >
          {{ $t("advance.menu.affectionMasterToggle") }}
        </Toggle>
      </MenuItem>
    </div>

    <!-- 内置 TTS 教程 -->
    <div class="h-full cursor-pointer transition-all duration-300" @click="openGuide">
      <MenuItem :title="$t('advance.menu.guideTitle')" size="large">
        <template #header>
          <BookOpen :size="20" />
        </template>
        <p class="mb-3 min-h-17 text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.guideDesc") }}
        </p>
        <Button type="big" icon="advance" :icon_size="18">
          {{ $t("advance.menu.guideButton") }}
        </Button>
      </MenuItem>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  AudioLines,
  BookOpen,
  Cast,
  Cpu,
  Database,
  Heart,
  Mic,
  SlidersHorizontal,
  Languages,
  Wrench,
} from "lucide-vue-next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { useI18n } from "vue-i18n";
import { MenuItem } from "../../ui";
import { Button } from "../../base";
import Toggle from "@/components/base/widget/Toggle.vue";
import { SUPPORTED_LOCALES, setLocale, type AppLocale } from "@/locales";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { getEnvConfigByKey, saveEnvConfig } from "@/api/services/config";
import { onMounted, ref } from "vue";

const { locale, t } = useI18n();
const dialogStore = useDialogStore();

// 好感度系统总开关，与「其他高级设置→好感度」的 affection.enabled 是同一项，切换后重启生效
const affectionMasterEnabled = ref(true);
// 取消确认或操作失败时递增，强制重渲染 Toggle 以恢复开关视觉状态
const affectionMasterEpoch = ref(0);

onMounted(async () => {
  try {
    const item = await getEnvConfigByKey("affection.enabled");
    affectionMasterEnabled.value = item.value !== "false";
  } catch {
    affectionMasterEnabled.value = true;
  }
});

// 重启流程抄自 SettingsBackground 的 HDR 开关：确认 → 写配置 → relaunch
async function onAffectionMasterToggle(enabled: boolean) {
  const ok = await dialogStore.confirm(t("advance.menu.affectionRestartConfirm"));
  if (!ok) {
    affectionMasterEpoch.value++;
    return;
  }
  try {
    await saveEnvConfig({ "affection.enabled": String(enabled) });
    affectionMasterEnabled.value = enabled;
    await relaunch();
  } catch (e) {
    console.error("切换好感度系统失败:", e);
    affectionMasterEpoch.value++;
    dialogStore.alert(t("advance.menu.affectionRestartFailed"));
  }
}

const emit = defineEmits<{
  navigate: [tab: "llm" | "tts" | "asr" | "other" | "tools" | "cast" | "memory"];
}>();

// 内置 TTS 官方教程（LingBlog）
const TTS_GUIDE_URL =
  "https://slimeboyowo.github.io/LingBlog/blog/projects/ling-chat/develop/tts_guide";

const openGuide = () => {
  void openUrl(TTS_GUIDE_URL);
};
</script>

<style scoped>
/* 统一卡片尺寸:菜单卡片等高(撑满 grid 行)、描述区对齐、按钮贴底 */
:deep(.menu-item) {
  height: 100%;
  display: flex;
  flex-direction: column;
}

:deep(.menu-item .content) {
  flex: 1;
  display: flex;
  flex-direction: column;
}

:deep(.menu-item .content > :last-child) {
  margin-top: auto;
}
</style>
