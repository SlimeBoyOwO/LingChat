<template>
    <div v-for="(control, index) in props.controls" :key="index">
        <component
            :is="control.element"
            :type="control.type"
            :id="control.title"
            :class="control.class"
            @click="control.action"
            v-if="control.show"
        >
            {{ control.title }}
        </component>
    </div>
</template>
<script setup lang="ts">
import { defineProps } from "vue";

import { SettingPage } from "../../../../api/types/settings";

const props = defineProps<{ controls: SettingPage[] }>();
</script>
<style></style>
