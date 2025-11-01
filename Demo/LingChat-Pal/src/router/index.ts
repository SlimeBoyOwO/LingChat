// src/router/index.ts
import { createRouter, createWebHashHistory } from "vue-router";
import SecondWindow from "../components/views/Second.vue";
import Main from "../components/views/Main.vue";

const routes = [
  {
    path: "/",
    name: "Main",
    component: Main,
  },
  {
    path: "/second",
    name: "SecondWindow",
    component: SecondWindow,
  },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
