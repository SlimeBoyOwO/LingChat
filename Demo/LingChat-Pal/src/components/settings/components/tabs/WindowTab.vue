<template>
  <article class="w-full h-full flex flex-col">
    <!-- 完美保留你原有的 Header 风格 -->
    <header
      class="mb-6 flex items-end justify-between border-b-2 pb-2 transition-colors shrink-0"
      :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'"
    >
      <div>
        <h2
          class="text-xl font-black tracking-wide mb-1 transition-colors"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
        >
          主动对话行为
        </h2>
        <p
          class="text-xs font-medium transition-colors"
          :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
        >
          基于桌面环境感知的主动系统与日程策略配置
        </p>
      </div>
      <span
        class="text-4xl font-bold italic select-none font-mono transition-colors uppercase"
        :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'"
      >
        SET
      </span>
    </header>

    <div class="flex flex-col pb-4 flex-1 gap-3">
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
        class="p-5 rounded-xl border shadow-sm md:col-span-2 flex items-center justify-between transition-colors duration-300 mt-2 shrink-0"
        :class="
          isDarkMode
            ? 'bg-slate-800/80 border-slate-700'
            : 'bg-slate-50 border-slate-200'
        "
      >
        <div class="flex flex-col gap-1.5">
          <span
            class="text-[10px] font-mono font-bold text-indigo-400 tracking-wider"
          >
            ACTION LOGIC
          </span>
          <h3
            class="font-bold text-[15px] transition-colors"
            :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
          >
            重载并应用配置
          </h3>
          <p
            class="text-xs transition-colors"
            :style="saveStatus.message ? { color: saveStatus.color } : {}"
            :class="
              !saveStatus.message
                ? isDarkMode
                  ? 'text-slate-400'
                  : 'text-slate-500'
                : ''
            "
          >
            {{
              saveStatus.message ||
              "点击保存后，将向底层服务同步最新环境变量并重启主动系统。"
            }}
          </p>
        </div>

        <!-- 将原有的圆形 Anchor 换成同样风格的 Save 按钮 -->
        <button
          @click="saveSettings"
          class="h-12 px-6 rounded-full border flex items-center justify-center shadow-sm transition-all hover:scale-105 active:scale-95 font-bold text-sm cursor-pointer"
          :class="
            isDarkMode
              ? 'bg-slate-700 border-indigo-900/50 text-indigo-400 hover:bg-slate-600'
              : 'bg-white border-indigo-100 text-indigo-500 hover:bg-indigo-50'
          "
        >
          <Save class="w-5 h-5 mr-2" />
          保存
        </button>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from "vue";
import { Save } from "lucide-vue-next";
import {
  getEnvConfigByKey,
  saveEnvConfigSettings,
} from "../../../../api/services/config";
import { reloadProactiveSystem } from "../../../../api/services/schedule";
import type { ConfigItem } from "../../../../api/services/config";
import SettingItem from "../../../base/items/SettingsItem.vue";

defineProps<{
  isDarkMode: boolean;
}>();

const settings = ref<Record<string, ConfigItem>>({});
const saveStatus = reactive({
  message: "",
  color: "#10b981", // 成功颜色
});

const saveSettings = async () => {
  const formData: Record<string, string> = {};
  Object.entries(settings.value).forEach(([key, config]) => {
    formData[key] = config.value;
  });

  saveStatus.message = "正在保存...";
  saveStatus.color = "#6366f1"; // 靛蓝色提示

  try {
    saveStatus.message = (await saveEnvConfigSettings(formData)).message;
    saveStatus.color = "#10b981";
    reloadProactiveSystem();
    await loadConfig();
  } catch (error: any) {
    saveStatus.message = `错误: ${error.message}`;
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
    "VD_API_KEY",
    "VD_BASE_URL",
    "VD_MODEL",
    "ENABLE_VISUAL_PRECEPTION",
    "ENABLE_TOPIC_CREATER",
    "ENABLE_TODO_PRECEPTION",
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
