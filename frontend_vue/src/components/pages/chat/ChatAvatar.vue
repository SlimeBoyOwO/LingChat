<template>
    <img id="avatar" :src="gameStatus.current.avatar" />
</template>

<script setup lang="ts">
import { gameStatus } from "../../../api/store";
import { Position } from "../../../api/types/Bubble";

import "./avatar-animation.css";
</script>

<style scoped>
#avatar {
    position: relative;
    width: fit-content;
    height: fit-content;
    background-size: contain;
    background-position: center center;
    background-repeat: no-repeat;
    overflow: show;
}

#avatar::before {
    /*TODO NEED_TEST*/
    content: v-bind("gameStatus.current.bubble?.content");
    position: relative;
    top: 0;
    left: 0;
    background-image: url("v-bind('gameStatus.current.bubble?.image')");
    background-size: contain;
    background-position: center center;
    background-repeat: no-repeat;
    z-index: 1;
    transition: background-image 0.2s ease-in-out;
    display: v-bind("gameStatus.current.bubble?.position === Position.Left ? 'block' : 'none'");
}
#avatar::after {
    /*TODO NEED_TEST*/
    content: v-bind("gameStatus.current.bubble?.content");
    position: relative;
    right: 0;
    top: 0;
    background-image: url("v-bind('gameStatus.current.bubble?.image')");
    background-size: contain;
    background-position: center center;
    background-repeat: no-repeat;
    z-index: 1;
    transition: background-image 0.2s ease-in-out;
    display: v-bind("gameStatus.current.bubble?.position === Position.Right ? 'block' : 'none'");
}
</style>
