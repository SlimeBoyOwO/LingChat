<template>
    <audio
        ref="audioRef"
        :src="props.source?.src"
        :loop="props.source?.loop"
        :volume="props.source?.volume"
        @ended="props.source?.onEnded"
    />
</template>
<script setup lang="ts">
import { ref, watch } from "vue";

import { SingleAudio } from "../../api/services/UIStatus";

const props = defineProps<{ source?: SingleAudio }>();

const audioRef = ref<HTMLAudioElement | null>(null);
watch(
    () => props.source?.src,
    () => {
        if (audioRef.value) {
            audioRef.value.load();
            if (props.source?.src) audioRef.value.play();
        }
    }
);
</script>
<style scoped>
audio {
    display: none;
}
</style>
