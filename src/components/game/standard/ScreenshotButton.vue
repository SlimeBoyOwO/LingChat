<template>
  <div
    class="group relative inline-flex"
    @mouseenter="onEnter"
    @mouseleave="previewVisible = false"
  >
    <!--
      预览必须 Teleport 到 body，否则永远看不见：
      对话框正文是 <div class="overflow-y-auto">（GameDialog 顶部），按 CSS 规范
      overflow-y:auto 会把 overflow-x 也算成 auto —— 两轴都裁剪。而预览是向上溢出
      按钮行（bottom-full），正好落在裁剪区之外，因此此前从未显示过。

      Teleport 到 body 后脱离 #app（useZoom 在 #app 上挂了 transform），
      position:fixed 相对视口定位，与 getBoundingClientRect 的坐标系一致。
    -->
    <Teleport to="body">
      <div
        v-if="hasScreenshot"
        class="pointer-events-none fixed z-[9999] transition-opacity duration-200"
        :class="previewVisible ? 'opacity-100' : 'opacity-0'"
        :style="previewStyle"
      >
        <img
          :src="'data:image/jpeg;base64,' + screenshotBase64"
          class="max-h-64 max-w-96 rounded-lg border-2 object-contain shadow-lg"
          style="border-color: var(--accent-color); background: #000"
        />
      </div>
    </Teleport>

    <Button
      type="nav"
      icon="camera"
      :title="hasScreenshot ? $t('game.dialog.screenshotRetake') : $t('game.dialog.screenshotAsk')"
      :style="hasScreenshot ? { color: 'var(--accent-color)' } : {}"
      @click="emit('start')"
      @contextmenu.prevent="emit('clear')"
    ></Button>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { Button } from "../../base";
import { useScreenshot } from "../../../composables/useScreenshot";

// 截图监听的注册/注销由父组件 GameDialog 统一负责（init/destroy）——
// 本组件可能因移动端菜单开合而频繁挂卸，不适合承担监听生命周期
const { hasScreenshot, screenshotBase64 } = useScreenshot();

const emit = defineEmits<{ start: []; clear: [] }>();

const previewVisible = ref(false);
const previewStyle = ref<Record<string, string>>({});

/** 悬停时按相机按钮的实际位置把预览贴到它正上方（等价于原来的 bottom-full mb-2） */
function onEnter(e: MouseEvent) {
  if (!hasScreenshot.value) return;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  previewStyle.value = {
    left: `${rect.left + rect.width / 2}px`,
    top: `${rect.top - 8}px`,
    transform: "translate(-50%, -100%)",
  };
  previewVisible.value = true;
}
</script>
