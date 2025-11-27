<template>
  <div
    class="message-item"
    v-for="message in dialog"
  >
    <div 
      class="player-message"
      v-if="message.type === 'message'"
    >
      <DialogUser
        :name="message.character"
        :content="message.content"
      />
    </div>
    <div
      class="chararter-reply"
      v-else-if="message.type === 'reply'"
    >
      <DialogCharacter
        :name ="message.character"
        :content="message.content"
        :emotionTag="message.originalTag"
        :emotionText="message.motionText"
        @click="rephrase(message.audioFile)"
      />
    </div>
    <div
      class="final-spacer"
      v-if="message.isFinal"
    ></div>
  </div>
  <audio ref="audio" autoplay></audio>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { API_CONFIG } from "../../../controllers/core/config";
import type { DialogMessage } from "../../../stores/modules/game/state";
import DialogUser from './DialogUser.vue';
import DialogCharacter from './DialogCharacter.vue';

const props = defineProps({
  dialog: {
    type: Array as () => DialogMessage[],
    require: true,
  },
});

const audio = ref<HTMLAudioElement | null>(null);

function rephrase(audioFile: string | undefined) {
  audio.value!.src = `${API_CONFIG.VOICE.BASE}/${audioFile}`;
  audio.value!.play();
}

</script>

<style scoped>

.message-item {
  line-height: 1.2;
  word-wrap: break-word;
}

.play-message {
  cursor: pointer;
  display: inline-block; /* 让边框包裹内容 */
}
.character-reply {
  cursor: help;
  display: inline-block; /* 让边框包裹内容 */
}

/* 为 isFinal 消息的间隔元素添加样式 */
.final-spacer {
  height: 1em; /* 高度约等于一个空行 */
}

</style>