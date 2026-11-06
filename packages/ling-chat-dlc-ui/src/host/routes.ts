import type { RouteRecordRaw } from "vue-router";
export const extensionRoutes: RouteRecordRaw[] = [
  {
    path: "/mini-games",
    name: "MiniGames",
    component: () => import("@lingchat/host/components/views/MainMenu.vue"),
    props: { initialMenu: "miniGames" },
  },
  {
    path: "/mini-games/:plugin/:game",
    name: "DlcGamePlayer",
    component: () => import("../components/views/dlc/GamePlayer.vue"),
  },
];
