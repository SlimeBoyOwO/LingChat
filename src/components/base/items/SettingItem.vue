<template>
  <!-- Case: 布尔值 (Checkbox) -->
  <template v-if="setting.type === 'bool'">
    <label
      class="text-brand inline-flex cursor-pointer items-center font-medium"
      :for="setting.key"
      >{{ setting.description || "" }}</label
    >
    <div class="flex px-1 py-2.5 align-baseline">
      <Toggle :checked="setting.value.toLowerCase() === 'true'" @change="handleCheckboxChange">
      </Toggle>
      <p class="text-sm text-gray-300">
        {{ setting.key }}
      </p>
    </div>
  </template>

  <!-- Case: 文本域 (Textarea) -->
  <template v-else-if="setting.type === 'textarea'">
    <label
      class="text-brand inline-flex cursor-pointer items-center font-medium"
      :for="setting.key"
      >{{ setting.description || $t("ui.settingItem.multilineHint") }}</label
    >
    <p class="mt-1 mb-2 text-sm text-gray-300">
      {{ setting.key }}
    </p>
    <textarea
      :id="setting.key"
      v-model="localValue"
      class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border
        border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white backdrop-blur-xl
        backdrop-saturate-150 transition-all duration-200 focus:ring-2 focus:outline-none"
      rows="8"
    ></textarea>
  </template>

  <!-- Case: 默认文本 (Text Input) -->
  <template v-else>
    <label
      class="text-brand inline-flex cursor-pointer items-center font-medium"
      :for="setting.key"
      >{{ setting.description || "" }}</label
    >
    <p class="mt-1 mb-2 text-sm text-gray-300">
      {{ setting.key }}
    </p>
    <!-- 如果是 path 类型，添加文件选择按钮 -->
    <div v-if="setting.type === 'path'" class="flex gap-2">
      <input
        type="text"
        :id="setting.key"
        v-model="localValue"
        class="shadow-glass focus:border-brand focus:ring-brand/20 flex-1 rounded-lg border
          border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white backdrop-blur-xl
          backdrop-saturate-150 transition-all duration-200 focus:ring-2 focus:outline-none"
      />
      <button
        @click="selectFile"
        type="button"
        class="bg-brand rounded-lg px-4 py-2.5 whitespace-nowrap text-white transition-colors
          duration-200 hover:bg-[#0056b3]"
      >
        {{ $t("ui.settingItem.browse") }}
      </button>
    </div>
    <div v-else-if="setting.type === 'number'">
      <input
        type="number"
        :id="setting.key"
        v-model="localValue"
        :min="setting.key === 'llm.timeout_secs' ? 10 : undefined"
        :max="setting.key === 'llm.timeout_secs' ? 3600 : undefined"
        :step="setting.key === 'llm.timeout_secs' ? 1 : undefined"
        class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border
          border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white backdrop-blur-xl
          backdrop-saturate-150 transition-all duration-200 focus:ring-2 focus:outline-none"
      />
    </div>
    <div v-else>
      <input
        type="text"
        :id="setting.key"
        v-model="localValue"
        class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border
          border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white backdrop-blur-xl
          backdrop-saturate-150 transition-all duration-200 focus:ring-2 focus:outline-none"
      />
    </div>
  </template>
</template>

<script setup lang="ts">
  import { ref, watch } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import Toggle from "../widget/Toggle.vue";

  interface Setting {
    key: string;
    value: string;
    type: "bool" | "textarea" | "text" | "path" | "number";
    description?: string;
  }

  interface Props {
    setting: Setting;
  }

  const props = defineProps<Props>();

  const emit = defineEmits<{
    "update:value": [value: string];
  }>();

  const localValue = ref(props.setting.value);

  // 监听本地值的变化，并触发更新事件
  watch(localValue, (newValue) => {
    emit("update:value", newValue);
  });

  // 监听props.setting.value的变化，同步到本地值
  watch(
    () => props.setting.value,
    (newValue) => {
      localValue.value = newValue;
    }
  );

  // 处理复选框的变化
  const handleCheckboxChange = (checked: boolean) => {
    const newValue = checked ? "true" : "false";
    localValue.value = newValue;
    emit("update:value", newValue);
  };

  const selectFile = async () => {
    try {
      const path = await invoke<string | null>("select_file");
      if (path) {
        // 通过 localValue 触发 watch → emit 'update:value' 回传父组件，
        // 让父组件写回原始配置对象（而不是改 localizedSetting 复制出的新对象）
        localValue.value = path;
      }
    } catch (error: any) {
      console.error("文件选择失败:", error);
    }
  };
</script>
