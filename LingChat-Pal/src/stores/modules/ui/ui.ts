// stores/ui.ts
import { defineStore } from "pinia";

interface UIState {
  showCharacterTitle: string;
  showCharacterSubtitle: string;
  showCharacterEmotion: string;
  showSettings: boolean;
  currentSettingsTab: string;
  typeWriterSpeed: number;
  enableChatEffectSound: boolean;
  currentBackground: string;
  currentBackgroundEffect: string;
  currentAvatarAudio: string;
  characterVolume: number;
  backgroundVolume: number;
  bubbleVolume: number;
  autoMode: boolean;
}

export const useUIStore = defineStore("ui", {
  state: (): UIState => ({
    showCharacterTitle: "Lovely You",
    showCharacterSubtitle: "Bilibili",
    showCharacterEmotion: "",
    showSettings: false,
    currentSettingsTab: "schedule",
    typeWriterSpeed: 50,
    enableChatEffectSound: true,
    currentBackground: "@/assets/images/default_bg.jpg",
    currentBackgroundEffect: "StarField",
    currentAvatarAudio: "None",
    characterVolume: 80,
    backgroundVolume: 80,
    bubbleVolume: 80,
    autoMode: false,
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
