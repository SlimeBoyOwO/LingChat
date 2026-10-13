<template>
  <StartList class="mini-game-options" :aria-label="t('views.menu.miniGame')">
    <p v-if="loading" class="menu-note" role="status">{{ t("views.dlc.working") }}</p>
    <p v-else-if="error" class="menu-note" role="alert">{{ error }}</p>
    <p v-else-if="!games.length" class="menu-note">{{ t("views.dlc.playEmpty") }}</p>

    <StartLine v-for="item in currentGames" :key="`${item.pluginId}/${item.game.id}`">
      <StartItem
        class="menu-subitem"
        @click="
          router.push({
            name: 'DlcGamePlayer',
            params: { plugin: item.pluginId, game: item.game.id },
          })
        "
      >
        {{ item.game.name }}
      </StartItem>
    </StartLine>

    <StartLine v-if="totalPages > 1" class="page-controls">
      <StartItem class="menu-subitem" :disabled="currentPage === 1" @click="currentPage--">
        &lt;
      </StartItem>
      <span class="page-count">{{ currentPage }} / {{ totalPages }}</span>
      <StartItem class="menu-subitem" :disabled="currentPage === totalPages" @click="currentPage++">
        &gt;
      </StartItem>
    </StartLine>
    <StartLine>
      <StartItem class="menu-subitem" @click="emit('back')">{{ t("views.menu.back") }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { StartItem, StartLine, StartList } from "../base";
import { listPlugins, type PluginInfo } from "@/api/services/plugins";

const emit = defineEmits<{ back: [] }>();
const { t } = useI18n();
const router = useRouter();
const plugins = ref<PluginInfo[]>([]);
const error = ref("");
const loading = ref(true);
const currentPage = ref(1);
const pageSize = 3;
const games = computed(() =>
  plugins.value
    .filter((plugin) => plugin.enabled && !plugin.error)
    .flatMap((plugin) => plugin.games.map((game) => ({ pluginId: plugin.id, game }))),
);
const totalPages = computed(() => Math.max(1, Math.ceil(games.value.length / pageSize)));
const currentGames = computed(() =>
  games.value.slice((currentPage.value - 1) * pageSize, currentPage.value * pageSize),
);

onMounted(async () => {
  try {
    plugins.value = await listPlugins();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
});
</script>

<style scoped>
.menu-note {
  max-width: min(560px, 78vw);
  color: white;
  font-size: clamp(18px, 2vw, 26px);
  line-height: 1.7;
  text-shadow: 0 2px 4px #0006;
  overflow-wrap: anywhere;
}
.page-controls {
  display: flex;
  align-items: baseline;
  gap: 24px;
}
.page-count {
  color: white;
  font-size: 28px;
  text-shadow: 0 2px 4px #0006;
}
</style>
