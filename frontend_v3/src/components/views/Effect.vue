<template>
    <div class="effect">
        <canvas class="effect-canvas" ref="canvas" />
        <AudioPlayer :src="effect?.audio" />
    </div>
</template>
<script setup lang="ts">
import { useTemplateRef, watch } from "vue";

import { IEffect } from "../../api/types/Effect.ts";
import { AudioPlayer } from "./index.ts";

const { effect } = defineProps<{ effect?: IEffect }>();
const canvas = useTemplateRef("canvas");
watch([() => effect, () => canvas], () => {
    if (canvas.value && effect) effect.initialize(canvas.value);
});
</script>
<style scoped>
.effect-canvas {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    pointer-events: none;
}
</style>
