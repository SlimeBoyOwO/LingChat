<template>
  <div class="dlc-library">
    <section class="catalog-controls">
      <p>{{ t("views.dlc.subtitle") }}</p>
      <div class="catalog-actions">
        <button :disabled="busy" @click="sync">{{ t("views.dlc.sync") }}</button>
        <button :disabled="busy || !updates.length" @click="updateInstalled">
          {{ t("views.dlc.updateAll") }}
        </button>
        <button :disabled="busy" @click="importArchive">{{ t("views.dlc.import") }}</button>
      </div>
    </section>
    <p v-if="error" role="alert" class="error">{{ error }}</p>
    <p v-if="notice" role="status">{{ notice }}</p>
    <div v-if="busy" role="status" class="progress">
      {{ t("views.dlc.working") }} <span v-if="progress">{{ progress }}%</span>
      <progress v-if="progress" :value="progress" max="100" />
    </div>
    <p v-if="!cards.length && !busy" class="empty">{{ t("views.dlc.empty") }}</p>
    <section class="game-grid">
      <article v-for="card in cards" :key="card.id">
        <div class="card-heading">
          <h2>{{ card.name }}</h2>
          <span class="badge">DLC</span>
        </div>
        <p class="description">{{ card.description }}</p>
        <p class="version">
          {{ t("views.dlc.version") }} {{ card.remote?.version || card.installed?.version }}
          <span v-if="card.installed">
            · {{ t("views.dlc.installed") }} {{ card.installed.version }}</span
          >
          <span v-if="card.remote"> · {{ (card.remote.size / 1024 / 1024).toFixed(1) }} MB</span>
        </p>
        <p v-if="card.installed?.error" class="error">{{ card.installed.error }}</p>
        <div class="actions">
          <button
            v-if="card.remote && !card.installed"
            class="primary"
            :disabled="busy"
            @click="install(card.id)"
          >
            {{ t("views.dlc.install") }}
          </button>
          <button
            v-else-if="
              card.remote &&
              card.installed &&
              newerGameVersion(card.remote.version, card.installed.version)
            "
            :disabled="busy"
            @click="install(card.id)"
          >
            {{ t("views.dlc.update") }}
          </button>
          <template v-if="card.installed">
            <button :disabled="busy || !!card.installed.error" @click="toggle(card.installed)">
              {{ t(card.installed.enabled ? "views.dlc.disable" : "views.dlc.enable") }}
            </button>
            <button
              v-for="game in card.installed.games"
              :key="game.id"
              class="primary"
              :disabled="busy || !card.installed.enabled || !!card.installed.error"
              @click="play(card.id, game)"
            >
              {{ t("views.dlc.play") }} · {{ game.name }}
            </button>
            <button :disabled="busy" @click="remove(card.id)">
              {{ t("views.dlc.uninstall") }}
            </button>
          </template>
        </div>
        <template v-if="card.installed">
          <button
            v-for="game in card.installed.games.filter((g) => g.legacy_storage_keys.length)"
            :key="'legacy-' + game.id"
            class="text-button"
            :disabled="busy"
            @click="migrate(card.id, game)"
          >
            {{ t("views.dlc.migrate") }} · {{ game.name }}
          </button>
        </template>
      </article>
    </section>
    <footer>{{ t("views.dlc.saveNote") }}</footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  cachedGameCatalog,
  newerGameVersion,
  syncGameCatalog,
  installGameDlc,
  openDlcGame,
  seedGameSave,
  type GameCatalogEntry,
  type GameSpec,
} from "@/api/services/games";
import {
  listPlugins,
  setPluginEnabled,
  deletePlugin,
  importPluginFromPath,
  type PluginInfo,
} from "@/api/services/plugins";
import { currentDeviceId } from "@/utils/audioOutputManager";

const emit = defineEmits<{ changed: [] }>();
const { t, locale } = useI18n();
const packages = ref<GameCatalogEntry[]>([]);
const installed = ref<PluginInfo[]>([]);
const busy = ref(false),
  error = ref(""),
  notice = ref(""),
  progress = ref(0);
let unlisten: UnlistenFn | undefined;
let disposed = false;
const cards = computed(() => {
  const entries = new Map<
    string,
    {
      id: string;
      name: string;
      description: string;
      remote?: GameCatalogEntry;
      installed?: PluginInfo;
    }
  >();
  for (const remote of packages.value) entries.set(remote.id, { ...remote, remote });
  for (const item of installed.value) {
    const card = entries.get(item.id) || {
      id: item.id,
      name: item.name,
      description: item.description,
    };
    entries.set(item.id, { ...card, installed: item });
  }
  return [...entries.values()];
});
const updates = computed(() =>
  cards.value.filter(
    (c) => c.installed && c.remote && newerGameVersion(c.remote.version, c.installed.version),
  ),
);
async function refresh() {
  installed.value = (await listPlugins()).filter((p) => p.games?.length);
}
async function run(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  progress.value = 0;
  try {
    await action();
  } catch (e) {
    error.value = String(e).includes("DLC_MULTI_WINDOW_UNSUPPORTED")
      ? t("views.dlc.unsupported")
      : String(e);
  } finally {
    busy.value = false;
    progress.value = 0;
  }
}
const sync = () =>
  run(async () => {
    const cache = await syncGameCatalog();
    packages.value = cache.catalog.packages;
    await refresh();
    notice.value = t("views.dlc.synced");
  });
const install = (id: string) =>
  run(async () => {
    await installGameDlc(id);
    await refresh();
    notice.value = t("views.dlc.installedNote");
  });
const updateInstalled = () =>
  run(async () => {
    try {
      for (const card of updates.value) await installGameDlc(card.id);
    } finally {
      await refresh();
    }
    notice.value = t("views.dlc.updated");
  });
const toggle = (plugin: PluginInfo) =>
  run(async () => {
    await setPluginEnabled(plugin.id, !plugin.enabled);
    await refresh();
  });
const play = (id: string, game: GameSpec) =>
  run(() => openDlcGame(id, game.id, locale.value, currentDeviceId.value));
const remove = (id: string) =>
  run(async () => {
    if (
      !(await confirm(t("views.dlc.uninstallConfirm"), {
        title: t("views.dlc.title"),
        kind: "warning",
      }))
    )
      return;
    await deletePlugin(id);
    await refresh();
  });
const importArchive = () =>
  run(async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "DLC", extensions: ["zip", "7z"] }],
    });
    if (typeof path !== "string") return;
    await importPluginFromPath({ path, conflict: "abort" });
    await refresh();
    emit("changed");
    notice.value = t("views.dlc.installedNote");
  });
const migrate = (id: string, game: GameSpec) =>
  run(async () => {
    if (
      !(await confirm(t("views.dlc.migrateConfirm") + "\n" + game.legacy_storage_keys.join("\n"), {
        title: game.name,
      }))
    )
      return;
    const value: Record<string, string> = {};
    for (const key of game.legacy_storage_keys) {
      const item = localStorage.getItem(key);
      if (item !== null) value[key] = item;
    }
    if (!Object.keys(value).length) {
      notice.value = t("views.dlc.noLegacy");
      return;
    }
    await seedGameSave(id, game.id, value);
    notice.value = t("views.dlc.migrated");
  });
onMounted(() => {
  void run(async () => {
    const cache = await cachedGameCatalog();
    packages.value = cache.catalog.packages;
    await refresh();
    const stop = await listen<{ received: number; total: number }>(
      "dlc:download-progress",
      (event) => {
        progress.value = Math.round((event.payload.received / event.payload.total) * 100);
      },
    );
    if (disposed) stop();
    else unlisten = stop;
  });
});
onBeforeUnmount(() => {
  disposed = true;
  unlisten?.();
});
</script>

<style scoped>
.dlc-library {
  width: 100%;
  min-width: 0;
  color: inherit;
}
.catalog-controls,
.catalog-actions,
.card-heading,
.actions {
  display: flex;
  align-items: center;
  gap: 14px;
}
.catalog-controls,
.catalog-actions {
  flex-wrap: wrap;
}
.catalog-controls > p {
  flex: 1;
  min-width: 180px;
}
h2 {
  font-size: 21px;
  margin: 0;
}
p {
  margin: 8px 0;
}
.catalog-controls p,
.version,
footer {
  color: #acc2cc;
  font-size: 13px;
}
.catalog-controls,
.game-grid,
footer,
.empty,
.error,
.progress,
.dlc-library > p {
  margin: 16px auto;
}
button {
  color: inherit;
  background: #304b5b;
  border: 1px solid #5d7988;
  border-radius: 9px;
  padding: 9px 15px;
  cursor: pointer;
  white-space: nowrap;
}
button:hover:enabled {
  background: #426276;
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
button.primary {
  color: #112c30;
  background: #8ce0d0;
  border-color: #8ce0d0;
}
button.primary:hover:enabled {
  background: #b4f3e8;
}
.game-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr));
  gap: 18px;
}
article {
  padding: 20px;
  background: #ffffff08;
  border: 1px solid #7396ab55;
  border-radius: 18px;
}
.card-heading {
  justify-content: space-between;
}
.badge {
  font-size: 11px;
  letter-spacing: 2px;
  color: #8ce0d0;
}
.description {
  min-height: 44px;
  color: #c9dce4;
  line-height: 1.65;
}
.actions {
  margin-top: 22px;
  flex-wrap: wrap;
}
.error {
  color: #ffb5b5;
  overflow-wrap: anywhere;
}
.text-button {
  background: transparent;
  border: 0;
  color: #acc2cc;
  font-size: 12px;
  margin-top: 12px;
  padding-left: 0;
}
progress {
  display: block;
  width: 100%;
  margin-top: 8px;
}
.empty {
  padding: 60px 0;
  text-align: center;
}
@media (max-width: 640px) {
  article {
    padding: 18px;
  }
}
</style>
