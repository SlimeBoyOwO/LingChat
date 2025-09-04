<template>
    <div class="chat-container">
        <div class="chat-info">
            <span id="name">{{ gameStatus.current.name }}</span>
            <span id="subtitle">{{ gameStatus.current.subtitle }}</span>
            <span id="emotion">{{ gameStatus.current.emotion }}</span>
            <Button type="nav" icon="history" title="" @click="openHistory"></Button>
        </div>
        <hr />
        <div class="chat-input">
            <textarea
                id="inputMessage"
                :placeholder="gameStatus.current.placeholder"
                v-model.lazy.trim="gameStatus.current.text"
                @keydown.enter.exact.prevent="sendOrContinue"
                :readonly="gameStatus.current.status !== AIStatus.IDLE"
            />
            <button id="sendButton" :disabled="gameStatus.current.status !== AIStatus.IDLE" @click="sendOrContinue">
                ▼
            </button>
        </div>
        <div class="chat-button-container">
            <template v-for="item in chat_buttons">
                <Button
                    class="chat-button"
                    v-if="item.visibility"
                    :icon="item.icon"
                    :key="item.order"
                    @click="item.action"
                    :disabled="!item.enable"
                >
                    {{ item.label }}
                </Button>
            </template>
        </div>
    </div>
</template>

<script setup lang="ts">
import { Ref, ref } from "vue";

import { PAGES } from "../../../../api/consts";
import { AIStatus } from "../../../../api/services/GameStatus";
import { gameStatus, i18n, uiStatus } from "../../../../api/store";
// import { chatHandler } from "../../../api/websocket/handlers/chat-handler";
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
.chatbox-box {
    position: relative;
    display: flex;
    justify-content: center;
    width: 100%;
    z-index: 2;
    background: linear-gradient(to top, rgba(0, 14, 39, 0.7) 0%, rgba(0, 14, 39, 0.6) 100%);
    padding: 15px;
    backdrop-filter: blur(1px);
}

.chatbox-box::before {
    content: "";
    position: absolute;
    top: -40px;
    left: 0;
    right: 0;
    height: 40px;
    background: linear-gradient(to bottom, transparent 0%, rgba(0, 14, 39, 0.3) 50%, rgba(0, 14, 39, 0.6) 100%);
    pointer-events: none;
}

.chatbox-main {
    width: 60%;
}

.chatbox-title-part {
    display: flex;
    align-items: baseline;
    margin-bottom: 10px;
}

/* 确保所有文本元素都继承相同的字体和文字阴影 */
.chatbox-title,
.chatbox-subtitle,
#inputMessage,
#sendButton {
    font-family: inherit;
    /* 继承父元素字体 */
    text-shadow: inherit;
    /* 继承文字阴影 */
}

/* 调整特定元素的字体大小和粗细 */
.chatbox-title {
    font-size: 24px;
    font-weight: bold;
    color: white;
    margin-right: 15px;
}

.chatbox-subtitle {
    font-size: 20px;
    font-weight: bold;
    color: #6eb4ff;
}

.chatbox-emotion {
    font-size: 20px;
    font-weight: bold;
    color: #ff77dd;
    margin: auto;
}

.chatbox-line {
    height: 1px;
    background: rgba(255, 255, 255, 0.3);
    margin: 6px 0 6px 0;
}

.chatbox-inputbox {
    display: flex;
    flex-direction: column;
    white-space: pre-line;
    width: 100%;
    min-height: 40px;
    background: rgba(255, 255, 255, 0);
    border: none;
    color: white;
    font-size: 20px;
    font-weight: bold;
    resize: none;
    margin: 5px 0px;
    outline: none;
    transition: all 0.3s;
}

#inputMessage {
    width: 100%;
    min-height: 40px;
    background: rgba(255, 255, 255, 0);
    border: none;
    color: white;
    font-size: 20px;
    font-weight: bold;
    resize: none;
    margin: 5px 0px;
    outline: none;
    transition: all 0.3s;
}

#inputMessage::placeholder {
    color: rgba(255, 255, 255, 0.5);
    /* 明亮的灰色 */
    text-shadow: none;
    /* 移除阴影 */
}

#sendButton {
    align-self: flex-end;
    background: rgba(0, 14, 39, 0);
    color: rgb(4, 188, 255);
    border: none;
    padding: 4px 10px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 0.3s;
    font-size: 20px;
    font-weight: bold;
    transform: scaleX(1.5);
}

#sendButton:hover {
    background: rgba(0, 14, 39, 0);
    color: rgba(136, 255, 251, 0.827);
}

#sendButton:disabled {
    background: #333;
    cursor: not-allowed;
    opacity: 0.7;
}
</style>
