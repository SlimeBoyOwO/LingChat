<template>
  <div
    class="p-2 min-h-30 rounded-xl border shadow-sm relative overflow-hidden group transition-colors duration-300 flex flex-col justify-between h-full"
    :class="isDarkMode
      ? 'bg-slate-800/50 border-slate-700'
      : 'bg-white border-slate-200'
      ">
    <!-- 左侧状态强调条 -->
    <div class="absolute top-0 left-0 w-1 h-full transition-colors duration-300"
      :class="themeConfig[setting.type].barClass"></div>

    <!-- 背景装饰图标 -->
    <component :is="themeConfig[setting.type].icon"
      class="absolute -right-4 -bottom-4 w-24 h-24 rotate-12 pointer-events-none transition-colors duration-500"
      :class="isDarkMode ? 'opacity-10' : 'opacity-[0.03]'" :style="{ color: themeConfig[setting.type].colorHex }" />

    <!-- 卡片内容区 -->
    <div class="flex flex-col gap-1 pl-2 relative z-10 h-full">
      <!-- 标签与类型 -->
      <div class="flex items-center gap-1.5 mb-2">
        <component :is="themeConfig[setting.type].icon" class="w-4 h-4" :class="themeConfig[setting.type].textClass" />
        <span class="text-[10px] font-mono font-bold tracking-wider uppercase"
          :class="themeConfig[setting.type].textClass">
          {{ setting.type }} TYPE
        </span>
      </div>

      <!-- 标题与描述 -->
      <div class="flex gap-1.5 mb-2 items-end">
        <label :for="setting.key" class="font-bold text-[14px] transition-colors wrap-break-word cursor-pointer"
          :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'">
          {{ setting.description || getDefaultDescription(setting.type) }}
        </label>
        <p class="text-xs leading-relaxed transition-colors line-clamp-2"
          :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'">
          {{ setting.key }}
        </p>
      </div>

      <!-- 控件区域 (自动撑开底部) -->
      <div class="">
        <!-- Case: 布尔值 (Checkbox/Toggle) -->
        <template v-if="setting.type === 'bool'">
          <div class="flex items-center mt-2 pt-2">
            <Toggle :checked="setting.value.toLowerCase() === 'true'" @change="handleCheckboxChange" />
          </div>
        </template>

        <!-- Case: 文本域 (Textarea) -->
        <template v-else-if="setting.type === 'textarea'">
          <textarea :id="setting.key" v-model="localValue" rows="4"
            class="w-full px-3 py-2 border rounded-lg text-sm transition-all duration-200 focus:outline-none focus:ring-2 resize-none"
            :class="[
              isDarkMode
                ? 'bg-slate-900/50 border-slate-600 text-slate-200 focus:border-sky-500 focus:ring-sky-500/20'
                : 'bg-slate-50 border-slate-200 text-slate-800 focus:border-sky-500 focus:ring-sky-500/20',
            ]"></textarea>
        </template>

        <!-- Case: 路径 (Path) -->
        <template v-else-if="setting.type === 'path'">
          <div class="flex gap-2">
            <input type="text" :id="setting.key" v-model="localValue"
              class="flex-1 px-3 py-2 border rounded-lg text-sm transition-all duration-200 focus:outline-none focus:ring-2 min-w-0"
              :class="[
                isDarkMode
                  ? 'bg-slate-900/50 border-slate-600 text-slate-200 focus:border-orange-500 focus:ring-orange-500/20'
                  : 'bg-slate-50 border-slate-200 text-slate-800 focus:border-orange-500 focus:ring-orange-500/20',
              ]" />
            <button @click="selectFile()" type="button"
              class="px-4 py-2 flex items-center justify-center rounded-lg text-xs font-bold tracking-wider text-white transition-colors duration-200 whitespace-nowrap"
              :class="isDarkMode
                ? 'bg-orange-600 hover:bg-orange-500'
                : 'bg-orange-500 hover:bg-orange-600'
                ">
              浏览
            </button>
          </div>
        </template>

        <!-- Case: 默认文本 (Text Input) -->
        <template v-else>
          <input type="text" :id="setting.key" v-model="localValue"
            class="w-full px-3 py-2 border rounded-lg text-sm transition-all duration-200 focus:outline-none focus:ring-2"
            :class="[
              isDarkMode
                ? 'bg-slate-900/50 border-slate-600 text-slate-200 focus:border-indigo-500 focus:ring-indigo-500/20'
                : 'bg-slate-50 border-slate-200 text-slate-800 focus:border-indigo-500 focus:ring-indigo-500/20',
            ]" />
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from "vue";
import {
  ToggleLeft,
  AlignLeft,
  TextCursorInput,
  FolderSearch,
} from "lucide-vue-next";
import Toggle from "../widget/Toggle.vue"; // 假设你有这个组件

interface Setting {
  key: string;
  value: string;
  type: "bool" | "textarea" | "text" | "path";
  description?: string;
}

const props = defineProps<{
  setting: Setting;
  isDarkMode: boolean;
}>();

const emit = defineEmits<{
  "update:value": [value: string];
}>();

const localValue = ref(props.setting.value);

// 监听本地值
watch(localValue, (newValue) => {
  emit("update:value", newValue);
});

// 监听props值同步
watch(
  () => props.setting.value,
  (newValue) => {
    localValue.value = newValue;
  },
);

// 处理复选框
const handleCheckboxChange = (checked: boolean) => {
  const newValue = checked ? "true" : "false";
  localValue.value = newValue;
  emit("update:value", newValue);
};

// 选择文件
const selectFile = async () => {
  try {
    const response = await fetch("/api/settings/select-file");
    const result = await response.json();
    if (result.error) {
      console.error("后端错误:", result.error);
    } else if (result.path) {
      localValue.value = result.path; // 使用 localValue 触发 update 事件
    }
  } catch (error: any) {
    console.error("文件选择失败:", error);
  }
};

// 获取默认描述
const getDefaultDescription = (type: string) => {
  const map: Record<string, string> = {
    bool: "开启或关闭此功能项",
    textarea: "支持多行长文本输入",
    path: "请输入或选择本地文件路径",
    text: "请输入标准配置参数",
  };
  return map[type] || "配置参数项";
};

// 为不同类型的设置分配专属UI主题
const themeConfig = computed(() => {
  return {
    bool: {
      icon: ToggleLeft,
      textClass: "text-emerald-500",
      barClass: props.isDarkMode
        ? "bg-emerald-900 group-hover:bg-emerald-500"
        : "bg-emerald-200 group-hover:bg-emerald-400",
      colorHex: "#10b981",
    },
    textarea: {
      icon: AlignLeft,
      textClass: "text-sky-500",
      barClass: props.isDarkMode
        ? "bg-sky-900 group-hover:bg-sky-500"
        : "bg-sky-200 group-hover:bg-sky-400",
      colorHex: "#0ea5e9",
    },
    path: {
      icon: FolderSearch,
      textClass: "text-orange-500",
      barClass: props.isDarkMode
        ? "bg-orange-900 group-hover:bg-orange-500"
        : "bg-orange-200 group-hover:bg-orange-400",
      colorHex: "#f97316",
    },
    text: {
      icon: TextCursorInput,
      textClass: "text-indigo-500",
      barClass: props.isDarkMode
        ? "bg-indigo-900 group-hover:bg-indigo-500"
        : "bg-indigo-200 group-hover:bg-indigo-400",
      colorHex: "#6366f1",
    },
  };
});
</script>
