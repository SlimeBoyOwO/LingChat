<template>
    <Transition name="fast-loader-animation">
        <div id="fast-loader" v-if="uiStatus.isLoading && uiStatus.isFastLoad" class="fast-loader" />
    </Transition>
</template>
<script setup lang="ts">
import { uiStatus } from '../../api/store';
</script>
<style>
#fast-loader {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    -webkit-backdrop-filter: blur(5px);
    backdrop-filter: blur(5px);
    z-index: 9999;
    opacity: 1;
}

#fast-loader.fast-loader-animation-leave-active {
    animation: fadeOut 1s ease-out;
}

@keyframes fadeOut {

    0%,
    40% {
        -webkit-backdrop-filter: blur(5px);
        backdrop-filter: blur(5px);
    }

    90%,
    100% {
        -webkit-backdrop-filter: none;
        backdrop-filter: none;
    }
}
</style>