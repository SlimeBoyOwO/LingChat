<template>
  <div ref="host" class="twilight-rhythm-page overflow-auto" data-scrollable></div>
</template>

<script setup lang="ts">
  import { onBeforeUnmount, onMounted, ref } from "vue";
  import { useRouter } from "vue-router";
  import { mountRhythm, type RhythmController } from "@/minigames/twilight/game.js";
  import template from "@/minigames/twilight/template.html?raw";
  import styles from "@/minigames/twilight/style.css?raw";

  const router = useRouter();
  const host = ref<HTMLDivElement | null>(null);
  const lifetime = new AbortController();
  let controller: RhythmController | undefined;

  onMounted(async () => {
    if (!host.value) return;
    // Keep the game's pixel typography and effects separate from global menu skins.
    // The canvas and AudioContext still run in the main app, with its audio-device manager.
    const root = host.value.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    style.textContent = styles;
    root.innerHTML = template;
    root.prepend(style);
    controller = await mountRhythm(root, {
      signal: lifetime.signal,
      onExit: () => {
        void router.push("/mini-games");
      },
    });
  });

  onBeforeUnmount(() => {
    lifetime.abort();
    controller?.destroy();
  });
</script>

<style scoped>
  .twilight-rhythm-page {
    width: 100%;
    height: 100%;
    min-height: 0;
    background: #1b1721;
  }
</style>
