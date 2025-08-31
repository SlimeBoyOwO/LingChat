import { createApp } from "vue";
import { createPinia } from "pinia";
import { connectWebSocket } from "./api/websocket";

import App from "./App.vue";
import "./assets/styles/base.css";
import "./assets/styles/variables.css";

import router from "./router"; // './router/index.js' 的简写

const app = createApp(App);
connectWebSocket("ws://localhost:8765/ws");
app.use(createPinia());
app.use(router);
app.mount("#app");
