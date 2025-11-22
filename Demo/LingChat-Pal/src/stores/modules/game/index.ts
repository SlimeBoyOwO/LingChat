import { defineStore } from "pinia";
import { state } from "./state";
import { actions } from "./actions";
import { getters } from "./getters";

export const useGameStore = defineStore("game", {
  state: () => state,
  // cast to any to avoid strict getter signature mismatches (e.g. getters returning functions with params)
  getters: getters as any,
  actions: actions as any,
});
