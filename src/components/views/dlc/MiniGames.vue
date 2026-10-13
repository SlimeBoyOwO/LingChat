<template>
  <main class="mini-games">
    <header>
      <button @click="router.push('/')">← {{ t("views.menu.back") }}</button>
      <h1>{{ t("views.menu.miniGame") }}</h1>
    </header>
    <p v-if="error" role="alert">{{ error }}</p>
    <p v-if="loading" role="status">{{ t("views.dlc.working") }}</p>
    <p v-else-if="!games.length">{{ t("views.dlc.playEmpty") }}</p>
    <section>
      <article v-for="item in games" :key="`${item.plugin.id}/${item.game.id}`">
        <h2>{{ item.game.name }}</h2>
        <p>{{ item.plugin.description }}</p>
        <button
          @click="
            router.push({
              name: 'DlcGamePlayer',
              params: { plugin: item.plugin.id, game: item.game.id },
            })
          "
        >
          {{ t("views.dlc.play") }}
        </button>
      </article>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { listPlugins, type PluginInfo } from "@/api/services/plugins";
const { t } = useI18n();
const router = useRouter();
const plugins = ref<PluginInfo[]>([]);
const error = ref("");
const loading = ref(true);
const games = computed(() =>
  plugins.value
    .filter((p) => p.enabled && !p.error)
    .flatMap((plugin) => plugin.games.map((game) => ({ plugin, game }))),
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
.mini-games {
  min-height: 100dvh;
  padding: 36px clamp(20px, 8vw, 120px);
  box-sizing: border-box;
  background: #142b3b;
  color: #e4f2fa;
}
header {
  display: flex;
  align-items: center;
  gap: 24px;
  margin-bottom: 32px;
}
section {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
  gap: 24px;
}
article {
  background: #ffffff0a;
  padding: 28px;
  border: 1px solid #a1c6d644;
  border-radius: 18px;
}
article p {
  min-height: 64px;
  color: #b8d0dc;
  line-height: 1.7;
}
button {
  color: inherit;
  background: #30596d;
  border: 1px solid #7fa9bc;
  border-radius: 10px;
  padding: 12px 22px;
  cursor: pointer;
}
button:hover {
  background: #41738a;
}
h1 {
  margin: 0;
  font-size: 30px;
}
</style>
