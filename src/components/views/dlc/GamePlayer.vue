<template>
  <main class="game-player">
    <header>
      <button :disabled="opening || closing" @click="leave">← {{ t("views.menu.back") }}</button>
      <strong>{{ title }}</strong>
      <span v-if="opening || closing" role="status">{{ t("views.dlc.working") }}</span>
      <span v-if="error" role="alert">{{ error }}</span>
    </header>
    <div ref="viewport" class="viewport" />
  </main>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { onBeforeRouteLeave, useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openDlcGame } from "@/api/services/games";
import { listPlugins } from "@/api/services/plugins";
import { currentDeviceId } from "@/utils/audioOutputManager";
const { t, locale } = useI18n();
const route = useRoute(),
  router = useRouter();
const viewport = ref<HTMLElement>();
const title = ref(""),
  error = ref(""),
  opening = ref(true),
  closing = ref(false);
let active = false,
  disposed = false,
  observer: ResizeObserver | undefined,
  unlisten: UnlistenFn | undefined;
let resizeFrame = 0;
function bounds() {
  const rect = viewport.value!.getBoundingClientRect();
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}
async function close(): Promise<boolean> {
  if (!active) return true;
  closing.value = true;
  try {
    await invoke("game_close");
    active = false;
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
    title.value =
      (await listPlugins()).find((p) => p.id === plugin)?.games.find((g) => g.id === game)?.name ||
      game;
    unlisten = await listen("dlc:closed", () => {
      active = false;
      if (!disposed && !opening.value && !closing.value) void router.replace("/mini-games");
    });
    await nextTick();
    // Wait until the route transition has laid out the full main-window viewport.
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    await openDlcGame(plugin, game, locale.value, currentDeviceId.value, bounds());
    active = true;
    observer = new ResizeObserver(() => {
      cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => {
        if (active && !disposed)
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
  display: flex;
  flex-direction: column;
}
header {
  min-height: 54px;
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 6px 16px;
  box-sizing: border-box;
  flex-shrink: 0;
}
header span {
  font-size: 13px;
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
  flex: 1;
  min-height: 0;
}
</style>
