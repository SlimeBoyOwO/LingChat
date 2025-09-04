import { createApp } from "vue";

import { API_URLS } from "./api/consts";
import { connectWebSocket } from "./api/websocket";
import App from "./App.vue";

import "./assets/styles/base.css";
import "./assets/styles/variables.css";

const app = createApp(App);
connectWebSocket(API_URLS.WEBSOCKET);
app.mount("body");
