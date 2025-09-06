<template>
    <div class="chat-container">
        <div class="chat-content-area">
            <div class="chat-info">
                <span id="name">{{ gameStatus.current.name }}</span>
                <span id="subtitle">{{ gameStatus.current.subtitle }}</span>
                <span id="emotion">{{ gameStatus.current.emotion }}</span>
            </div>
            <textarea
                id="chat-message"
                :placeholder="i18n(gameStatus.current.placeholder)"
                v-model.lazy.trim="gameStatus.current.text"
                @keydown.enter.exact.prevent="sendOrContinue"
                :readonly="gameStatus.current.status !== AIStatus.IDLE"
            />
        </div>
        <div class="chat-button-container">
            <template v-for="item in chat_buttons">
                <IconButton
                    class="chat-button"
                    v-if="item.visibility"
                    :icon="item.icon"
                    :key="item.order"
                    @click="item.action"
                    :disabled="!item.enable"
                    :text="item.label"
                />
            </template>
        </div>
    </div>
</template>

<script setup lang="ts">
import { Ref, ref } from "vue";

import { PAGES } from "../../../../api/consts";
import { AIStatus } from "../../../../api/services/GameStatus";
import { gameStatus, i18n, uiStatus } from "../../../../api/store";
import { IconButton } from "../../../controls";
import { IconType } from "../../../controls/Icon.vue";

function openHistory() {
    uiStatus.value.settings.switchPage("history");
    uiStatus.value.main.switchPage("Settings").beginLoading();
}

function saveGame() {
    uiStatus.value.settings.switchPage(PAGES.SETTINGS.SAVE).with({ status: "save" });
    uiStatus.value.main.switchPage(PAGES.MAIN.SETTINGS).beginLoading();
}

function loadGame() {
    uiStatus.value.settings.switchPage(PAGES.SETTINGS.SAVE).with({ status: "load" });
    uiStatus.value.main.switchPage(PAGES.MAIN.SETTINGS).beginLoading();
}

const chat_buttons: {
    icon?: IconType;
    label: string;
    order: number;
    visibility: Ref<boolean>;
    enable: Ref<boolean>;
    action: () => void;
}[] = [
    {
        icon: undefined,
        label: i18n.value("chat.buttons.saveGame"),
        order: 1,
        visibility: ref(true),
        enable: ref(true),
        action: saveGame
    },
    {
        icon: undefined,
        label: i18n.value("chat.buttons.loadGame"),
        order: 2,
        visibility: ref(true),
        enable: ref(true),
        action: loadGame
    },
    {
        icon: undefined,
        label: i18n.value("chat.buttons.history"),
        order: 3,
        visibility: ref(true),
        enable: ref(true),
        action: openHistory
    }
];

const inputMessage = ref("");

function sendOrContinue() {
    if (gameStatus.value.current.status === AIStatus.IDLE) {
        send();
    } else if (gameStatus.value.current.status === AIStatus.RESPONDING) {
        continueDialog();
    }
}

function send() {
    if (!inputMessage.value.trim()) return;
    // chatHandler.sendMessage(inputMessage.value);
    inputMessage.value = "";
}

function continueDialog() {
    // chatHandler.continueMessage();
}
</script>

<style>
.chat-container {
    position: relative;
    width: 100%;
    height: 20%;
    display:flex;
    flex-direction:row;
    background: linear-gradient(to top, rgba(0, 14, 39, 0.7) 0%, rgba(0, 14, 39, 0.6) 100%);
    padding: 15px;
    backdrop-filter: blur(1px);
}

.chat-container::before {
    content: "";
    position: absolute;
    top: -40px;
    left: 0;
    right: 0;
    height: 40px;
    background: linear-gradient(to bottom, transparent 0%, rgba(0, 14, 39, 0.3) 50%, rgba(0, 14, 39, 0.6) 100%);
    pointer-events: none;
}
.chat-content-area {
    width:100%;
    margin-right:40px;
    padding: 0 20px;
}
.chat-info {
    color: white;
    padding-bottom: 5px;
    padding-left: 30px;
    border-bottom: 1px solid aqua;
    display:flex;
    align-items:baseline;
}

/* 调整特定元素的字体大小和粗细 */
.chat-info #name {
    font-size: 24px;
    font-weight: bold;
    color: white;
    margin-right: 2%;
}

.chat-info #subtitle {
    font-size: 20px;
    font-weight: bold;
    color: #6eb4ff;
    margin-right: 10%;
}

.chat-info #emotion {
    font-size: 20px;
    font-weight: bold;
    color: #ff77dd;
}
#chat-message {
    width: 100%;
    min-height: 40px;
    background: rgba(255, 255, 255, 0);
    border: none;
    color: white;
    font-size: 20px;
    font-weight: bold;
    resize: none;
    padding: 20px;
    outline: none;
    transition: all 0.3s;
}
.chat-button-container {
    height:100%;
    width:fit-content;
    display:flex;
    align-items:flex-end;
    flex-direction:column;
    flex-wrap:wrap;
}
.chat-button-container .chat-button {
    margin:5px 0;
    height:50px;
    width:120px;
    background: transparent;
    color: white;
    font-size: 20px;
    font-weight: bold;
    cursor: pointer;
    transition: all 0.3s;
    border-radius: 40px;
    background-color:rgba(255,192,203, 0.5);
}
#inputMessage::placeholder {
    color: rgba(255, 255, 255, 0.5);
    /* 明亮的灰色 */
    text-shadow: none;
    /* 移除阴影 */
}
</style>
