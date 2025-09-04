<template>
    <div id="global-background" :style="{ backgroundImage: `url(${uiStatus.main.background.image})` }">
        <Effect id="background-effect" :effect="uiStatus.main.background.effect" />
    </div>
    <AudioPlayer :src="uiStatus.main.background.music" />
</template>
<script setup>
import { AudioPlayer } from ".";
import { uiStatus } from "../../api/store";
import Effect from "./Effect.vue";
</script>
<style scoped>
#global-background {
    width: 100vw;
    height: 100vh;
    position: fixed;
    left: 0;
    top: 0;
    pointer-events: none;
    background-size: cover;
    background-position: center center;
    background-repeat: none;
}
</style>
