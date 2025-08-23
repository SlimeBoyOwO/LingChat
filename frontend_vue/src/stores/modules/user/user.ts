import { defineStore } from "pinia";

export const useUserStore = defineStore("ui", {
  state: () => ({
    user_id: "1",
  }),
  actions: {},
});
