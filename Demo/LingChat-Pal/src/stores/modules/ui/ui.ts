// stores/ui.ts
import { defineStore } from "pinia";

interface UIState {
  showCharacterTitle: string;
  showCharacterSubtitle: string;
  showCharacterEmotion: string;
  showCharacterLine: string;
  showPlayerHintLine: string;
  showCharacterThinkLine: string;
  showSettings: boolean;
  currentSettingsTab: string;
  typeWriterSpeed: number;
  enableChatEffectSound: boolean;
  currentBackground: string;
  currentBackgroundEffect: string;
  currentBackgroundMusic: string;
  currentSoundEffect: string;
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
    showCharacterLine: "",
    showPlayerHintLine: "",
    showCharacterThinkLine: "Ling Ling Thinking...",
    showSettings: false,
    currentSettingsTab: "text",
    typeWriterSpeed: 50,
    enableChatEffectSound: true,
    currentBackground: "@/assets/images/default_bg.jpg",
    currentBackgroundEffect: "StarField",
    currentBackgroundMusic: "None",
    currentSoundEffect: "None",
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
