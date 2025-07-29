// stores/ui.ts
import { defineStore } from "pinia";

export const useUIStore = defineStore("ui", {
  state: () => ({
    showCharacterTitle: "Lovely You",
    showCharacterSubtitle: "Bilibili",
    showCharacterEmotion: "",
    showSettings: false,
    currentSettingsTab: "text", // 可以添加当前选中的设置标签页
    typeWriterSpeed: 50,
    enableChatEffectSound: true,
  }),
  actions: {
    toggleSettings(show: boolean) {
      this.showSettings = show;
    },
    setSettingsTab(tab: string) {
      this.currentSettingsTab = tab;
    },
  },
});
