// src/main.ts
import { createApp } from "vue";
import { createPinia } from "pinia";
import { connectWebSocket } from "./api/websocket";
import { initializeEventProcessors } from "./core/events";
import App from "./App.vue";
import router from "./router";

const pinia = createPinia();
const app = createApp(App);

import "./api/websocket/handlers/script-handler";

connectWebSocket("ws://localhost:8765/ws");

initializeEventProcessors();

app.use(pinia);
app.use(router);
app.mount("#app");
