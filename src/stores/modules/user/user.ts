import { defineStore } from "pinia";

export const useUserStore = defineStore("user", {
  state: () => ({
    user_id: "1",
    client_id: "",
  }),
  actions: {},
});
