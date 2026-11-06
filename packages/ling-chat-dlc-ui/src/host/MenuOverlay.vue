<template>
  <!-- 剧本可持久化的预设式标题异常层：只渲染纯文本/CSS，不接受 HTML 或资源路径。 -->
  <div v-if="effect.theme !== 'normal'" class="script-menu-effect" aria-hidden="true">
    <div class="script-menu-effect__scanlines"></div>
    <div v-if="effect.message" class="script-menu-effect__message">
      {{ effect.message }}
    </div>
  </div>

  <!-- DLC 识别提示（右下角小字；有已识别 DLC 时才显示） -->
  <div v-if="visible && names.length > 0" class="dlc-hint">
    {{ $t("views.mainMenu.dlcRecognized", { names: names.join("、") }) }}
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { MenuEffect } from "./menu";
defineProps<{ effect: MenuEffect; names: string[]; visible: boolean }>();
const { t: $t } = useI18n();
</script>
<style src="../styles/menu-effects.css"></style>
<style scoped>
/* DLC 识别提示：右下角半透明黄小字，不挡菜单 */
.dlc-hint {
  position: absolute;
  right: 14px;
  bottom: 30px;
  z-index: 5;
  font-size: 12px;
  letter-spacing: 0.05em;
  color: rgba(255, 214, 90, 0.78);
  text-shadow:
    0 1px 4px rgba(0, 0, 0, 0.7),
    0 0 2px rgba(0, 0, 0, 0.45);
  pointer-events: none;
}
</style>
