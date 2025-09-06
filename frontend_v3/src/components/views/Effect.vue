<template>
    <div class="effect">
        <component :is="effect?.type" />
        <AudioPlayer :src="effect?.audio" />
    </div>
</template>
<script setup lang="ts">
import { IEffect } from "../../api/types/Effect.ts";
import { AudioPlayer } from "./index.ts";

const { effect } = defineProps<{ effect?: IEffect }>();
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
