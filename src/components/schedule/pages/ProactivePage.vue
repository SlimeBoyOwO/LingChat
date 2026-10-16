<template>
  <!-- 视图：日程主题列表 -->
  <div
    v-if="uiStore.scheduleView === 'proactive_settings'"
    class="grid grid-cols-1 p-1 sm:grid-cols-1 lg:grid-cols-1"
  >
    <div v-for="setting in settings" :key="setting.key" class="mb-6">
      <!-- 提醒方式是二选一，SettingItem 只支持 bool/text/textarea/path/number，这里单独渲染 -->
      <template v-if="setting.key === 'TODO_REMINDER_STYLE'">
        <label class="text-brand inline-flex items-center font-medium">
          {{ $t("ui.proactivePage.todoReminderStyle") }}
        </label>
        <p class="mt-1 mb-2 text-sm text-gray-300">{{ setting.key }}</p>
        <div class="flex gap-2">
          <div
            v-for="option in reminderStyleOptions"
            :key="option.value"
            @click="setting.value = option.value"
            class="cursor-pointer rounded-lg px-4 py-2.5 text-sm transition-colors duration-200"
            :class="
              setting.value === option.value
                ? 'bg-brand text-white'
                : 'border border-white/10 bg-white/10 text-white hover:bg-white/20'
            "
          >
            {{ option.label }}
          </div>
        </div>
      </template>
      <!-- 使用 SettingItem 组件渲染不同类型的输入控件 -->
      <SettingItem v-else :setting="setting" @update:value="(value) => (setting.value = value)" />
      <!-- 期望间隔是个抽象数字，这里回显它反推出来的兴趣增长范围 -->
      <p
        v-if="setting.key === 'PROACTIVE_EXPECTED_INTERVAL_MINUTES' && growthRange"
        class="mt-2 text-sm text-gray-400"
      >
        {{
          $t("ui.proactivePage.growthRangeHint", {
            min: growthRange[0].toFixed(2),
            max: growthRange[1].toFixed(2),
          })
        }}
      </p>
    </div>

    <div class="flex h-auto w-auto items-center gap-2 align-text-bottom">
      <div
        class="bg-brand w-18 cursor-pointer rounded-lg border-none px-5 py-2.5 text-sm font-medium text-white transition-colors duration-200 hover:bg-[#0056b3]"
        @click="saveSettings"
      >
        {{ $t("ui.proactivePage.save") }}
      </div>
      <p :style="{ color: saveStatus.color }">
        {{ saveStatus.message }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive, computed, watch } from "vue";
import { useUIStore } from "@/stores/modules/ui/ui";
import { getEnvConfigByKey, saveEnvConfigSettings } from "@/api/services/config";
import { previewProactiveInterestRange, reloadProactiveSystem } from "@/api/services/schedule";
import type { ConfigItem } from "@/api/services/config";
import SettingItem from "@/components/base/items/SettingItem.vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const uiStore = useUIStore();
const settings = ref<Record<string, ConfigItem>>({});
const saveStatus = reactive({
  message: "",
  color: "#4ade80",
});

const reminderStyleOptions = computed(() => [
  { value: "role", label: t("ui.proactivePage.todoReminderStyleRole") },
  { value: "toast", label: t("ui.proactivePage.todoReminderStyleToast") },
]);

const INTERVAL_KEY = "PROACTIVE_EXPECTED_INTERVAL_MINUTES";
const growthRange = ref<[number, number] | null>(null);

// 换算在后端（唯一真相源），这里只负责把结果贴出来
const refreshGrowthRange = async () => {
  const minutes = Number(settings.value[INTERVAL_KEY]?.value);
  if (!Number.isFinite(minutes) || minutes <= 0) {
    growthRange.value = null;
    return;
  }
  try {
    growthRange.value = await previewProactiveInterestRange(minutes);
  } catch (error: any) {
    console.error("兴趣增长范围预览失败:", error.message);
    growthRange.value = null;
  }
};

watch(() => settings.value[INTERVAL_KEY]?.value, refreshGrowthRange);

const saveSettings = async () => {
  // 将 settings 转换为 Record<string, string> 格式
  const formData: Record<string, string> = {};
  Object.entries(settings.value).forEach(([key, config]) => {
    formData[key] = config.value;
  });

  saveStatus.message = "";

  try {
    await saveEnvConfigSettings(formData);
    // 保存成功后实时重载主动系统，让新配置立即生效
    await reloadProactiveSystem();
    saveStatus.message = t("ui.proactivePage.success");
    saveStatus.color = "#4ade80";

    await loadConfig();
  } catch (error: any) {
    saveStatus.message = t("ui.proactivePage.error", { message: error.message });
    saveStatus.color = "red";
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
    INTERVAL_KEY,
    "ENABLE_VISUAL_PRECEPTION",
    "SCREEN_WEIGHT",
    "ENABLE_TOPIC_CREATER",
    "TOPIC_WEIGHT",
    "ENABLE_TODO_PRECEPTION",
    "TODO_WEIGHT",
    "ENABLE_SCHEDULE_REMINDER",
    "ENABLE_IMPORTANT_DAY_REMINDER",
    "FORCE_SCHEDULE_REMINDER",
    "FORCE_TODO_REMINDER",
    "TODO_REMINDER_STYLE",
  ];

  for (const key of configKeys) {
    settings.value[key] = await getEnvConfigByKey(key);
  }

  await refreshGrowthRange();
};

onMounted(async () => {
  await loadConfig();
});
</script>
