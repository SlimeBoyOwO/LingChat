<template>
  <button
    class="director-tab director-tab-handle"
    :class="{ 'director-tab-hidden': store.open }"
    :title="$t('game.director.title')"
    @click="openPanel"
  >
    <PawPrint :size="16" />
    <span class="director-tab-label">{{ $t("game.director.title") }}</span>
  </button>

  <Transition name="director-slide">
    <aside v-if="store.open" class="director-panel" :style="panelStyle">
      <button
        v-if="!narrow"
        class="director-tab director-tab-panel"
        :title="$t('game.director.title')"
        @click="store.open = false"
      >
        <PawPrint :size="16" />
        <span class="director-tab-label">{{ $t("game.director.title") }}</span>
      </button>

      <div v-if="!narrow" class="director-grip" @pointerdown="onGripDown"></div>

      <header class="flex shrink-0 items-center gap-2 px-4 pt-3 pb-2">
        <PawPrint :size="13" :class="statusIconClass" />
        <span class="text-[0.86rem] font-semibold text-white/90">{{
          $t("game.director.title")
        }}</span>
        <span class="min-w-0 truncate text-[0.64rem] text-white/40">{{ saveLabel }}</span>
        <span class="ml-auto shrink-0 text-[0.64rem] text-white/45">{{ statusLabel }}</span>
        <button
          class="ml-0.5 shrink-0 rounded-md p-1 text-white/40 transition-colors hover:bg-white/10 hover:text-white/85"
          @click="store.open = false"
        >
          <X :size="14" />
        </button>
      </header>

      <nav class="flex shrink-0 gap-1 px-3 pb-2">
        <button
          v-for="tab in TABS"
          :key="tab.key"
          class="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[0.74rem] transition-colors duration-200"
          :class="
            store.activeTab === tab.key
              ? 'bg-white/12 text-white/90'
              : 'text-white/50 hover:bg-white/6 hover:text-white/75'
          "
          @click="store.activeTab = tab.key"
        >
          <component :is="tab.icon" :size="13" class="shrink-0" />
          <span>{{ $t(tab.label) }}</span>
        </button>
      </nav>

      <div class="min-h-0 flex-1">
        <DirectorChatTab v-if="store.activeTab === 'chat'" />
        <DirectorContextTab v-else-if="store.activeTab === 'context'" />
        <DirectorToolHistoryTab v-else-if="store.activeTab === 'tools'" />
        <DirectorDraftTab v-else />
      </div>
    </aside>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { Brain, Clapperboard, MessageSquare, PawPrint, Wrench, X } from "@lucide/vue";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useGameStore } from "@/stores/modules/game";
import {
  DIRECTOR_MAX_WIDTH,
  DIRECTOR_MIN_WIDTH,
  useDirectorStore,
  type DirectorTab,
} from "@/stores/modules/director";
import DirectorChatTab from "./DirectorChatTab.vue";
import DirectorContextTab from "./DirectorContextTab.vue";
import DirectorToolHistoryTab from "./DirectorToolHistoryTab.vue";
import DirectorDraftTab from "./DirectorDraftTab.vue";

const { t } = useI18n();
const store = useDirectorStore();
const uiStore = useUIStore();
const gameStore = useGameStore();

const TABS: { key: DirectorTab; label: string; icon: Component }[] = [
  { key: "chat", label: "game.director.tabs.chat", icon: MessageSquare },
  { key: "context", label: "game.director.tabs.context", icon: Brain },
  { key: "tools", label: "game.director.tabs.tools", icon: Wrench },
  { key: "draft", label: "game.director.tabs.draft", icon: Clapperboard },
];

const narrow = ref(false);
const syncNarrow = () => (narrow.value = window.innerWidth < 768);

onMounted(() => {
  syncNarrow();
  window.addEventListener("resize", syncNarrow);
  void store.init();
});

let cleanupDrag: (() => void) | null = null;
onBeforeUnmount(() => {
  window.removeEventListener("resize", syncNarrow);
  cleanupDrag?.();
});

const panelStyle = computed(() => ({
  width: narrow.value ? "calc(100vw - 16px)" : `${store.width}px`,
}));

function onGripDown(e: PointerEvent) {
  if (narrow.value) return;
  e.preventDefault();
  cleanupDrag?.();
  const startX = e.clientX;
  const startW = store.width;
  const onMove = (ev: PointerEvent) => {
    const maxW = Math.min(DIRECTOR_MAX_WIDTH, Math.floor(window.innerWidth * 0.55));
    store.width = Math.min(maxW, Math.max(DIRECTOR_MIN_WIDTH, startW + startX - ev.clientX));
  };
  const onUp = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    if (cleanupDrag === onUp) cleanupDrag = null;
  };
  cleanupDrag = onUp;
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onUp);
}

const statusIconClass = computed(() => {
  switch (store.statusKind) {
    case "running":
      return "shrink-0 animate-pulse text-amber-300";
    case "idle":
      return "shrink-0 text-brand";
    default:
      return "shrink-0 text-white/25";
  }
});

const statusLabel = computed(() => {
  switch (store.statusKind) {
    case "running":
      return t("game.director.statusRunning");
    case "idle":
      return t("game.director.statusIdle");
    default:
      return t("game.director.statusUnavailable");
  }
});

const saveLabel = computed(() => {
  if (!store.saveBound || store.saveId == null) return t("game.director.saveFree");
  return store.saveTitle || t("game.director.saveBound", { id: store.saveId });
});

function openPanel() {
  store.open = true;
  void store.init();
}

watch(
  () => gameStore.activeSaveId,
  (saveId) => void store.switchSession(saveId),
);

watch(
  () => uiStore.showSettings,
  (open) => {
    if (open) {
      store.open = false;
      return;
    }
    void store.loadState();
  },
);
</script>

<style scoped>
.director-tab {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px 6px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-right: none;
  border-radius: 12px 0 0 12px;
  background: rgba(18, 18, 28, 0.75);
  color: rgba(255, 255, 255, 0.65);
  backdrop-filter: blur(16px);
  box-shadow: -4px 0 20px rgba(0, 0, 0, 0.35);
  transform: translateY(-50%);
  transition:
    color 0.2s,
    background 0.2s,
    opacity 0.3s cubic-bezier(0.32, 0.72, 0, 1);
}

.director-tab:hover {
  background: rgba(28, 28, 40, 0.88);
  color: rgba(255, 255, 255, 0.9);
}

.director-tab-handle {
  position: fixed;
  top: 50%;
  right: 0;
  z-index: 999;
}

.director-tab-panel {
  position: absolute;
  top: 50%;
  right: 100%;
  z-index: 2;
}

.director-tab-hidden {
  opacity: 0;
  pointer-events: none;
}

.director-tab-label {
  writing-mode: vertical-rl;
  letter-spacing: 0.3em;
  font-size: 0.72rem;
}

.director-panel {
  position: fixed;
  top: calc(74px + var(--safe-area-inset-top));
  right: 0;
  bottom: calc(16px + var(--safe-area-inset-bottom));
  z-index: 999;
  display: flex;
  flex-direction: column;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-right: none;
  border-radius: 18px 0 0 18px;
  background: rgba(18, 18, 28, 0.78);
  backdrop-filter: blur(20px);
  box-shadow: -8px 0 32px rgba(0, 0, 0, 0.4);
}

.director-panel ::-webkit-scrollbar {
  width: 4px;
  height: 4px;
}

.director-panel ::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.16);
  border-radius: 2px;
}

.director-panel ::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.3);
}

.director-slide-enter-active,
.director-slide-leave-active {
  transition:
    transform 0.36s cubic-bezier(0.32, 0.72, 0, 1),
    opacity 0.3s cubic-bezier(0.32, 0.72, 0, 1);
  will-change: transform;
}

.director-slide-enter-from,
.director-slide-leave-to {
  transform: translateX(100%);
  opacity: 0;
}

.director-grip {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  z-index: 1;
  width: 6px;
  cursor: ew-resize;
  touch-action: none;
  background: transparent;
  transition: background 0.2s;
}

.director-grip:hover {
  background: rgba(255, 255, 255, 0.12);
}

@media (max-width: 767px) {
  .director-panel {
    top: calc(64px + var(--safe-area-inset-top));
    right: 0;
    bottom: calc(8px + var(--safe-area-inset-bottom));
    border-radius: 14px 0 0 14px;
  }
}
</style>
