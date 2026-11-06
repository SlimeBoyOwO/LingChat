import { computed, ref, type Ref, type Component } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ScriptSummary } from "@lingchat/host/api/services/script-info";
import MiniGameOptions from "../components/views/menu/page/MiniGameOptions.vue";
import glitchLogo from "../assets/images/LingChatLogoGlitch.png";
export interface MenuEffect {
  theme: "normal" | "blood" | "ghost";
  message?: string;
  owner?: string;
}
export const menuPages: Record<string, Component> = {
  miniGames: MiniGameOptions,
};
export const menuItems = [{ key: "miniGames", label: "views.menu.miniGame" }];
export function useMenuPresentation(scripts: Ref<ScriptSummary[]>) {
  const effect = ref<MenuEffect>({ theme: "normal" });
  async function refresh() {
    try {
      effect.value = (await invoke<MenuEffect | null>("get_script_menu_effect")) ?? {
        theme: "normal",
      };
    } catch {
      effect.value = { theme: "normal" };
    }
  }
  return {
    effect,
    refresh,
    names: computed(() => scripts.value.filter((s) => s.plugin_id).map((s) => s.script_name)),
    className: computed(() => `main-menu-page--${effect.value.theme}`),
    particlesEnabled: computed(() => effect.value.theme === "normal"),
    logo: computed(() => (effect.value.theme === "normal" ? undefined : glitchLogo)),
  };
}
