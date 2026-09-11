<template>
  <article class="flex h-full w-full flex-col">
    <!-- 完美保留你原有的 Header 风格 -->
    <header
      class="mb-6 flex shrink-0 items-end justify-between border-b-2 pb-2 transition-colors"
      :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'"
    >
      <div>
        <h2
          class="mb-1 text-xl font-black tracking-wide transition-colors"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
        >
          {{ $t("pet.window.title") }}
        </h2>
        <p
          class="text-xs font-medium transition-colors"
          :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
        >
          {{ $t("pet.window.desc") }}
        </p>
      </div>
      <span
        class="font-mono text-4xl font-bold uppercase italic transition-colors select-none"
        :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'"
      >
        SET
      </span>
    </header>

    <div class="flex flex-1 flex-col gap-3 pb-4">
      <!-- 渲染设置项卡片 (复刻原图 1、2 号卡片风格) -->
      <SettingItem
        v-for="setting in settings"
        :key="setting.key"
        :setting="setting"
        :is-dark-mode="isDarkMode"
        @update:value="(value) => (setting.value = value)"
      />

      <!-- 保存操作栏：完美复刻你的第3个卡片 (Anchor Logic) 样式 -->
      <div
        class="mt-2 flex shrink-0 items-center justify-between rounded-xl border p-5 shadow-sm
          transition-colors duration-300 md:col-span-2"
        :class="isDarkMode ? 'border-slate-700 bg-slate-800/80' : 'border-slate-200 bg-slate-50'"
      >
        <div class="flex flex-col gap-1.5">
          <span class="font-mono text-[10px] font-bold tracking-wider text-indigo-400">
            ACTION LOGIC
          </span>
          <h3
            class="text-[15px] font-bold transition-colors"
            :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
          >
            {{ $t("pet.window.applyTitle") }}
          </h3>
          <p
            class="text-xs transition-colors"
            :style="saveStatus.message ? { color: saveStatus.color } : {}"
            :class="!saveStatus.message ? (isDarkMode ? 'text-slate-400' : 'text-slate-500') : ''"
          >
            {{ saveStatus.message || $t("pet.window.applyDesc") }}
          </p>
        </div>

        <!-- 将原有的圆形 Anchor 换成同样风格的 Save 按钮 -->
        <button
          @click="saveSettings"
          class="flex h-12 cursor-pointer items-center justify-center rounded-full border px-6
            text-sm font-bold shadow-sm transition-all hover:scale-105 active:scale-95"
          :class="
            isDarkMode
              ? 'border-indigo-900/50 bg-slate-700 text-indigo-400 hover:bg-slate-600'
              : 'border-indigo-100 bg-white text-indigo-500 hover:bg-indigo-50'
          "
        >
          <Save class="mr-2 h-5 w-5" />
          {{ $t("pet.window.save") }}
        </button>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
  import { ref, onMounted, reactive } from "vue";
  import { useI18n } from "vue-i18n";
  import { Save } from "lucide-vue-next";
  import { getEnvConfigByKey, saveEnvConfigSettings } from "../../../../api/services/config";
  import { reloadProactiveSystem } from "../../../../api/services/schedule";
  import type { ConfigItem } from "../../../../api/services/config";
  import SettingItem from "../../../base/items/SettingItem.vue";

  defineProps<{
    isDarkMode: boolean;
  }>();

  const settings = ref<Record<string, ConfigItem>>({});
  const { t } = useI18n();
  const saveStatus = reactive({
    message: "",
    color: "#10b981", // 成功颜色
  });

  const saveSettings = async () => {
    const formData: Record<string, string> = {};
    Object.entries(settings.value).forEach(([key, config]) => {
      formData[key] = config.value;
    });

    saveStatus.message = t("pet.window.saving");
    saveStatus.color = "#6366f1"; // 靛蓝色提示

    try {
      saveStatus.message = (await saveEnvConfigSettings(formData)).message;
      saveStatus.color = "#10b981";
      reloadProactiveSystem();
      await loadConfig();
    } catch (error: any) {
      saveStatus.message = t("pet.window.error", { message: error.message });
      saveStatus.color = "#ef4444";
    } finally {
      setTimeout(() => {
        saveStatus.message = "";
      }, 5000);
    }
  };

  const loadConfig = async () => {
    const configKeys = [
      "ENABLE_PROACTIVE_SYSTEM",
      "MAX_PROACTIVE_TIMES",
      "ENABLE_VISUAL_PRECEPTION",
      "SCREEN_WEIGHT",
      "ENABLE_TOPIC_CREATER",
      "TOPIC_WEIGHT",
      "ENABLE_TODO_PRECEPTION",
      "TODO_WEIGHT",
      "ENABLE_SCHEDULE_REMINDER",
      "ENABLE_IMPORTANT_DAY_REMINDER",
    ];

    for (const key of configKeys) {
      settings.value[key] = await getEnvConfigByKey(key);
    }
  };

  onMounted(async () => {
    loadConfig();
  });
</script>
