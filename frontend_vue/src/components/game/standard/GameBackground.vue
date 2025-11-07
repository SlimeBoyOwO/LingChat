<template>
  <div class="game-background" :style="backgroundStyle">
    <StarField
      ref="starfieldRef"
      v-if="uiStore.currentBackgroundEffect === `StarField`"
      :enabled="starfieldEnabled"
      :star-count="starCount"
      :scroll-speed="scrollSpeed"
      :colors="starColors"
      @ready="onStarfieldReady"
    />
    <Rain
      v-if="uiStore.currentBackgroundEffect === `Rain`"
      :enabled="rainEnabled"
      :intensity="rainIntensity"
    />
    <Sakura
      v-if="uiStore.currentBackgroundEffect === `Sakura`"
      :enabled="true"
      :intensity="1.5"
    >
    </Sakura>
    <
  </div>
  <audio ref="soundEffectPlayer"></audio>
  <audio ref="backgroundMusicPlayer" loop></audio>
</template>

<script setup>
import { ref, computed, watch } from "vue";
import { useUIStore } from "../../../stores/modules/ui/ui";
import StarField from "./particles/StarField.vue";
import Rain from "./particles/Rain.vue";
import Sakura from "./particles/Sakura.vue";

const uiStore = useUIStore();
const starfieldRef = ref(null);

const soundEffectPlayer = ref();
const backgroundMusicPlayer = ref();

// 星空效果控制
const starfieldEnabled = ref(true);
const starCount = ref(200);
const scrollSpeed = ref(0.4);
const starColors = ref([
  "rgb(173, 216, 230)",
  "rgb(176, 224, 230)",
  "rgb(241, 141, 252)",
  "rgb(176, 230, 224)",
  "rgb(173, 230, 216)",
]);

// 雨滴效果控制
const showRain = ref(false);
const rainEnabled = ref(true);
const rainIntensity = ref(1);

// 计算背景样式
const backgroundStyle = computed(() => {
  return {
    backgroundImage: uiStore.currentBackground
      ? `url(${uiStore.currentBackground})`
      : "url(@/assets/images/default_bg.jpg)",
  };
});

showRain.value = true;

// 星空就绪回调
const onStarfieldReady = (instance) => {
  console.log("Starfield ready", instance);
};

// 根据背景决定是否显示雨滴
watch(
  () => uiStore.currentBackgroundEffect,
  (newBackgroundEffect) => {
    if (newBackgroundEffect === "Rain") {
      showRain.value = true;
    } else if (newBackgroundEffect === "StarField") {
    }
  }
);

// 监听音效
watch(
  () => uiStore.currentSoundEffect,
  (newAudioUrl) => {
    if (soundEffectPlayer.value && newAudioUrl && newAudioUrl !== "None") {
      soundEffectPlayer.value.src = newAudioUrl;
      soundEffectPlayer.value.load();
      soundEffectPlayer.value.play();
    }
  }
);

// 监听音效
watch(
  () => uiStore.currentBackgroundMusic,
  (newAudioUrl) => {
    if (backgroundMusicPlayer.value && newAudioUrl && newAudioUrl !== "None") {
      backgroundMusicPlayer.value.src = newAudioUrl;
      backgroundMusicPlayer.value.load();
      backgroundMusicPlayer.value.play();
    }
  }
);
</script>

<style scoped>
.game-background {
  position: absolute;
  width: 100%;
  height: 100%;
  background-size: cover;
  background-position: center center;
  background-attachment: fixed;
  background-repeat: no-repeat;
  z-index: -2;
  transition: background-image 0.5s ease-in-out;
}
</style>
