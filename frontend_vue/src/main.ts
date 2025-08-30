import { createApp } from "vue";

import { API_URLS } from "./api/consts";
import { connectWebSocket } from "./api/websocket";
import App from "./App.vue";

import "./assets/styles/base.css";
import "./assets/styles/variables.css";

import router from "./router"; // './router/index.js' 的简写

const app = createApp(App);
connectWebSocket(API_URLS.WEBSOCKET);
app.use(router);
app.mount("body");
