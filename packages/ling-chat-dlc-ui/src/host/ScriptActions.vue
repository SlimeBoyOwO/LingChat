<template>
  <button
    v-if="script.has_persistent_vars"
    class="reset-memory-btn"
    type="button"
    :disabled="busy"
    @click.stop="$emit('reset')"
  >
    {{ done ? `✓ ${$t("views.menu.resetMemoryDone2")}` : $t("views.menu.resetMemory") }}
  </button>
</template>
<script setup lang="ts">
import type { ScriptSummary } from "@lingchat/host/api/services/script-info";
defineProps<{ script: ScriptSummary; busy: boolean; done: boolean }>();
defineEmits<{ reset: [] }>();
</script>
<style scoped>
/* 跟在大号剧本名按钮后面的记忆重置小字：与菜单同款白字阴影，只是字号小 */
.reset-memory-btn {
  margin-top: 26px;
  margin-left: 10px;
  padding: 4px 6px;
  vertical-align: middle;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.8);
  font-family:
    "Maoken_Assorted_Sans",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe_UI",
    Roboto,
    "Helvetica_Neue",
    Arial,
    sans-serif;
  font-size: clamp(14px, 1.4vw, 24px);
  line-height: 1.2;
  cursor: pointer;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.45);
  transition:
    color 0.25s ease,
    transform 0.25s ease,
    text-shadow 0.25s ease;
}

.reset-memory-btn:hover:not(:disabled) {
  color: #ff6b7a;
  transform: translateY(-2px);
  text-shadow: 0 0 6px rgba(255, 107, 122, 0.5);
}

.reset-memory-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
</style>
