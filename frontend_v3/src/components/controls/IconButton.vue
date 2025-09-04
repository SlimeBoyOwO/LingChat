<template>
    <button :class="[type, 'icon-button']" :disabled="disabled" @click="$emit('click')">
        <Icon v-if="icon" :icon="icon" :size="icon_size" />
        {{ i18n(props.text) }}
    </button>
</template>

<script setup lang="ts">
import { i18n } from "../../api/store";
import Icon, { type IconType } from "./Icon.vue";

interface ButtonProps {
    type?: "big" | "menu" | "nav" | "select" | "delete" | "add" | "save" | "start" | "close";
    disabled?: boolean;
    icon?: IconType;
    icon_size?: number;
    text: string;
}
const props = defineProps<ButtonProps>();
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const emit = defineEmits(["click"]);
</script>

<style scoped>
.icon-button {
    --bg-success: #4caf50;
    --bg-success-hover: #45a049;
    --bg-dismiss: #f44336;
    --bg-dismiss-hover: #d32f2f;
}
.icon-button {
    border: none;
    outline: none;
    cursor: pointer;
    font-weight: 500;
    padding: 8px 16px;
    border-radius: 4px;
    transition: all 0.2s ease;
}

/* 开始按钮样式 */
.icon-button.start {
    background-color: var(--bg-success);
    color: white;
}

.icon-button.start:hover {
    background-color: var(--bg-success-hover);
}
.icon-button.dismiss {
    background-color: var(--bg-dismiss);
    color: white;
}

.icon-button.dismiss:hover {
    background-color: var(--bg-dismiss-hover);
}

.icon-button.disabled {
    opacity: 0.6;
    cursor: not-allowed;
}

.icon-button svg {
    width: 18px;
    height: 18px;
    stroke-width: 2.5px;
    flex-shrink: 0;
}

.icon-button.big {
    width: 100%;
    padding: 12px;
    font-size: 16px;
    font-weight: bold;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    background-color: #e9ecef;
    color: #495057;
    transition: all 0.2s ease;
}

.icon-button.big:hover {
    background-color: var(--accent-color);
    color: white;
    transform: translateY(-2px);
    box-shadow: 0 4px 10px rgba(121, 217, 255, 0.4);
}

.icon-button.select {
    align-self: flex-end;
    /* 按钮靠右 */
    background-color: #ccc;
    color: #666;
    border: none;
    padding: 8px 15px;
    border-radius: 20px;
    cursor: pointer;
    transition: all 0.2s ease;
    font-size: 13px;
    font-weight: 500;
}

.icon-button.select.selected {
    background-color: var(--accent-color);
    color: white;
}

.icon-button.select:not(.selected):hover {
    background-color: #555;
    color: white;
}
</style>
