<template>
  <div class="grid grid-cols-1 gap-5 p-2 md:grid-cols-2 lg:grid-cols-3">
    <!-- 大模型管理 -->
    <div class="h-full cursor-pointer transition-all duration-300" @click="emit('navigate', 'llm')">
      <MenuItem :title="$t('advance.menu.llmTitle')" size="large">
        <template #header>
          <Cpu :size="20" />
        </template>
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
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
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
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
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
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
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
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
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
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
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.castDesc") }}
        </p>
        <Button type="big" icon="advance" :icon_size="18">
          {{ $t("advance.menu.castButton") }}
        </Button>
      </MenuItem>
    </div>

    <!-- 界面语言 -->
    <div class="h-full transition-all duration-300">
      <MenuItem :title="$t('advance.menu.languageTitle')" size="large">
        <template #header>
          <Languages :size="20" />
        </template>
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
          {{ $t("advance.menu.languageDesc") }}
        </p>
        <select
          :value="locale"
          class="w-full cursor-pointer rounded-lg border border-white/10 bg-white/5 px-3 py-2
            text-sm text-white/80 transition-all duration-200 hover:border-white/30 hover:text-white
            focus:border-[rgba(121,217,255,0.6)] focus:outline-none"
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

    <!-- 内置 TTS 教程 -->
    <div class="h-full cursor-pointer transition-all duration-300" @click="openGuide">
      <MenuItem :title="$t('advance.menu.guideTitle')" size="large">
        <template #header>
          <BookOpen :size="20" />
        </template>
        <p class="mb-3 min-h-[68px] text-sm leading-relaxed text-white/50">
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
    Mic,
    SlidersHorizontal,
    Languages,
    Wrench,
  } from "lucide-vue-next";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { useI18n } from "vue-i18n";
  import { MenuItem } from "../../ui";
  import { Button } from "../../base";
  import { SUPPORTED_LOCALES, setLocale, type AppLocale } from "@/locales";

  const { locale } = useI18n();

  const emit = defineEmits<{
    navigate: [tab: "llm" | "tts" | "asr" | "other" | "tools" | "cast"];
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
