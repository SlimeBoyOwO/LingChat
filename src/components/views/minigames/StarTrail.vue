<template>
  <div ref="host" class="star-trail-page"></div>
</template>

<script setup lang="ts">
  import { onBeforeUnmount, onMounted, ref } from "vue";
  import { useRouter } from "vue-router";
  import { mountStarTrail, type StarTrailController } from "@/minigames/star-trail/game.js";
  import template from "@/minigames/star-trail/template.html?raw";
  import styles from "@/minigames/star-trail/style.css?raw";

  const host = ref<HTMLDivElement | null>(null);
  const router = useRouter();
  const lifetime = new AbortController();
  let controller: StarTrailController | undefined;
  onMounted(() => {
    if (!host.value) return;
    const root = host.value.attachShadow({ mode: "open" });
    root.innerHTML = template;
    const style = document.createElement("style");
    style.textContent = styles;
    root.prepend(style);
    controller = mountStarTrail(root, {
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
  .star-trail-page {
    width: 100%;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: #173f64;
  }
</style>
