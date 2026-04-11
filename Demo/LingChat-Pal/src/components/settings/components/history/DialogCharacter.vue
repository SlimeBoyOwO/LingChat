<template>
  <div class="message" :title="`情感: ${emotionTag}`">
    <span class="name">{{ name }}</span>
    <span class="symbol">:&nbsp</span>
    <span class="content"
      >{{ content }}{{ action_content ? ` (${action_content})` : "" }}</span
    >
    <span class="symbol">&nbsp</span>
    <span class="icon"><Icon icon="volume" @click="$emit('click')" /></span>
  </div>
</template>

<script setup lang="ts">
import Icon from "../../../base/widget/Icon.vue";

const props = defineProps({
  name: {
    type: String,
    require: true,
  },
  content: {
    type: String,
    require: true,
  },
  action_content: {
    type: String,
    require: false,
  },
  emotionTag: {
    type: String,
    require: true,
  },
  emotionText: {
    type: String,
    require: true,
  },
});

const emit = defineEmits(["click"]);
</script>
<style scoped>
span {
  vertical-align: middle;
}

.name {
  font-weight: bold;
}

.symbol {
  font-weight: bold;
}
</style>
