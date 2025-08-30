<template>
    <Transition @after-enter="mounted">
        <div class="main-box" v-if="uiStatus.main.currentPage === 'ChatView'">
            <ChatAvatar />
            <ChatDialog />
            <div id="menu-panel">
                <Button type="nav" icon="text" @click="uiStatus.main.switchPage('settings')">
                    <h3>菜单</h3>
                </Button>
            </div>
        </div>
    </Transition>
</template>

<script setup lang="ts">
import { uiStatus } from "../../api/store";
import { Button } from "../base";
import { ChatAvatar, ChatDialog } from "../pages/chat";

function mounted() {
    uiStatus.value.main.endLoading();
}
</script>

<style>
.main-box {
    position: absolute;
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    overflow: hidden;
}

#menu-panel {
    position: fixed;
    top: 15px;
    right: 20px;
    z-index: 1000;
}
</style>
