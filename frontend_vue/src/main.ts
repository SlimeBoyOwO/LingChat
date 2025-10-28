import { createApp } from "vue";
import { createPinia } from "pinia";
import { connectWebSocket } from "./api/websocket";
import { initializeEventProcessors } from "./core/events";

import App from "./App.vue";
import "./assets/styles/base.css";
import "./assets/styles/variables.css";

import "./api/websocket/handlers/script-handler";

import router from "./router"; // './router/index.js' 的简写

const app = createApp(App);

connectWebSocket("ws://localhost:8765/ws");

initializeEventProcessors();

app.use(createPinia());
app.use(router);
app.mount("#app");
