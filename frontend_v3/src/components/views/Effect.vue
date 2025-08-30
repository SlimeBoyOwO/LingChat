<template>
    <canvas class="effect-canvas" ref="canvas" />
</template>
<script setup lang="ts">
import { useTemplateRef, watch } from "vue";

import { IEffect } from "../../api/types/Effect.ts";

const props = defineProps<{ effect: IEffect }>();
const canvas = useTemplateRef("canvas");
watch([() => props.effect, () => canvas], () => {
    if (canvas.value) props.effect.initialize(canvas.value);
});
</script>
<style scoped>
.effect-canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
}
</style>
