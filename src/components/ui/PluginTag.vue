<template>
  <span v-if="visible" class="plugin-tag" :title="tooltip">
    <Puzzle :size="11" />
    <span>{{ label }}</span>
  </span>
</template>

<script setup lang="ts">
  import { computed } from "vue";
  import { Puzzle } from "lucide-vue-next";

  const props = defineProps<{
    /** 列表项的 source 字段："game" 或插件 id。仅当非 game 时显示。 */
    source?: string | null;
    /** 显示用的插件名（可选），未提供时仅显示通用「插件」文案。 */
    pluginName?: string | null;
  }>();

  const visible = computed(() => !!props.source && props.source !== "game");
  const label = computed(() => (props.pluginName ? `插件·${props.pluginName}` : "插件"));
  const tooltip = computed(() =>
    props.pluginName
      ? `来自插件：${props.pluginName}`
      : "来自插件（运行时直读，可在插件管理页保留 / 隐藏）"
  );
</script>

<style scoped>
  .plugin-tag {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 1px 6px;
    border-radius: 9999px;
    font-size: 10px;
    line-height: 1.4;
    color: #c4b5fd;
    background: rgba(139, 92, 246, 0.16);
    border: 1px solid rgba(139, 92, 246, 0.35);
    white-space: nowrap;
  }
</style>
