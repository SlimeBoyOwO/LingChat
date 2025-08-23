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
    currentBackground: "@/assets/images/default_bg.jpg",
    currentAvatarAudio: "None",
    characterVolume: 50,
    backgroundVolume: 50,
    bubbleVolume: 50,
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
