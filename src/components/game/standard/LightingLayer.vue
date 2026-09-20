<template>
  <!--
    舞台光影层：压在立绘之上、UI 之下。

    这里放的是「整块画面级」的加法层——方向光、冷暖分离、暗角。它们靠
    mix-blend-mode 与下方已绘制的内容（背景 + 立绘）混合，所以必须位于立绘之后。

    刻意不套一层公共容器：混合是拿「最近的祖先层叠上下文」的背景来算的，容器一旦
    自成层叠上下文（z-index / filter / isolation），这几层就只会跟容器内的透明背景
    混合，等于完全看不见。z-index 写在本元素上是安全的——被混合的那层本来就必须
    自成层叠上下文。
  -->
  <div
    v-if="plan.directional"
    class="pointer-events-none absolute inset-0 z-20"
    :class="plan.directional.lit.className"
    :style="plan.directional.lit.style"
  ></div>
  <div
    v-if="plan.directional"
    class="pointer-events-none absolute inset-0 z-20"
    :class="plan.directional.shadow.className"
    :style="plan.directional.shadow.style"
  ></div>
  <div
    v-if="plan.grade"
    class="pointer-events-none absolute inset-0 z-20"
    :class="plan.grade.className"
    :style="plan.grade.style"
  ></div>
  <div
    v-if="plan.vignette"
    class="pointer-events-none absolute inset-0 z-20"
    :class="plan.vignette.className"
    :style="plan.vignette.style"
  ></div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useLightingStore } from "@/stores/modules/lighting";

const lightingStore = useLightingStore();
const plan = computed(() => lightingStore.plan);
</script>
