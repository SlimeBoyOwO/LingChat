import { createApp } from "vue";
import { createPinia } from "pinia";
import { connectWebSocket } from "./api/websocket";

import App from "./App.vue";
import "./assets/styles/base.css";
import "./assets/styles/variables.css";

const app = createApp(App);
connectWebSocket("ws://localhost:8765/ws");
app.use(createPinia());
app.mount("#app");
