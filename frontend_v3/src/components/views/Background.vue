<template>
    <div id="global-background" >
        <img :class="{blur:uiStatus.main.background.isBlur}" :src="uiStatus.main.background.image" />
        <Effect id="background-effect" :effect="uiStatus.main.background.effect" />
        <AudioPlayer :src="uiStatus.main.background.music" />
    </div>
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
}
#global-background img {
    width: auto;
    height: auto;
    position: absolute;
    pointer-events: none;
}
#global-background img.blur {
    filter: blur(8px);
    transition: filter 0.5s ease-in;
}
</style>
