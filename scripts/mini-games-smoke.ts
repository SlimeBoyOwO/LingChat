// Development-only route harness: mounts the actual menu and game Vue components.
import { createApp, h } from "vue";
import { createPinia } from "pinia";
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory, RouterView } from "vue-router";
import GameModeOptions from "../src/components/views/menu/page/GameModeOptions.vue";
import MiniGames from "../src/components/views/MiniGames.vue";
import TwilightRhythm from "../src/components/views/TwilightRhythm.vue";
import views from "../src/locales/zh-CN/views";
import "../src/assets/styles/base.css";

const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    { path: "/", component: GameModeOptions },
    { path: "/mini-games", component: MiniGames },
    { path: "/mini-games/twilight", component: TwilightRhythm },
  ],
});
const app = createApp({ render: () => h(RouterView) });
app.use(createPinia());
app.use(createI18n({ legacy: false, locale: "zh-CN", messages: { "zh-CN": { views } } }));
app.use(router);
app.mount("#app");
(window as any).__miniGameRouter = router;
