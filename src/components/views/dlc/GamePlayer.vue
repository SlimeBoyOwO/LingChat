<template>
  <main class="game-player">
    <div ref="viewport" class="viewport" />
    <div v-if="opening || (!active && error)" class="launch-status">
      <p v-if="opening" role="status">{{ t("views.dlc.working") }}</p>
      <template v-else>
        <p role="alert">{{ error }}</p>
        <button :disabled="closing" @click="leave">{{ t("views.menu.back") }}</button>
      </template>
    </div>
  </main>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { onBeforeRouteLeave, useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openDlcGame } from "@/api/services/games";
import { currentDeviceId } from "@/utils/audioOutputManager";
const { t, locale } = useI18n();
const route = useRoute(),
  router = useRouter();
const viewport = ref<HTMLElement>();
const error = ref(""),
  active = ref(false),
  opening = ref(true),
  closing = ref(false);
let disposed = false,
  observer: ResizeObserver | undefined,
  unlisten: UnlistenFn | undefined;
let resizeFrame = 0;
function bounds() {
  const rect = viewport.value!.getBoundingClientRect();
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}
async function close(): Promise<boolean> {
  if (!active.value) return true;
  closing.value = true;
  try {
    await invoke("game_close");
    active.value = false;
    return true;
  } catch (e) {
    error.value = String(e);
    return false;
  } finally {
    closing.value = false;
  }
}
async function leave() {
  if (await close()) await router.replace("/mini-games");
}
onBeforeRouteLeave(async () => !opening.value && (await close()));
onMounted(async () => {
  try {
    const plugin = String(route.params.plugin),
      game = String(route.params.game);
    unlisten = await listen("dlc:closed", () => {
      active.value = false;
      if (!disposed && !opening.value && !closing.value) void router.replace("/mini-games");
    });
    await nextTick();
    // Wait until the route transition has laid out the full main-window viewport.
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    await openDlcGame(plugin, game, locale.value, currentDeviceId.value, bounds());
    active.value = true;
    observer = new ResizeObserver(() => {
      cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => {
        if (active.value && !disposed)
          void invoke("game_resize", { bounds: bounds() }).catch((e) => {
            error.value = String(e);
          });
      });
    });
    observer.observe(viewport.value!);
  } catch (e) {
    error.value = String(e).includes("DLC_EMBED_UNSUPPORTED")
      ? t("views.dlc.unsupported")
      : String(e);
  } finally {
    opening.value = false;
  }
});
onBeforeUnmount(() => {
  disposed = true;
  observer?.disconnect();
  cancelAnimationFrame(resizeFrame);
  unlisten?.();
});
</script>

<style scoped>
.game-player {
  position: fixed;
  inset: 0;
  background: #101922;
  color: #e5f4fc;
}
.launch-status {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 18px;
  padding: 24px;
  box-sizing: border-box;
  overflow-wrap: anywhere;
}
[role="alert"] {
  color: #ffb6b6;
}
button {
  padding: 7px 18px;
  background: #2b485c;
  border: 1px solid #5f8499;
  border-radius: 8px;
  color: inherit;
  cursor: pointer;
  white-space: nowrap;
}
button:disabled {
  opacity: 0.5;
}
.viewport {
  position: absolute;
  inset: 0;
}
</style>
