import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
const calls: any[] = [];
mockWindows("main");
mockIPC(
  (cmd, args: any = {}) => {
    calls.push({ cmd, args });
    if (cmd === "get_role_info")
      return {
        character_id: args.roleId,
        ai_name: args.roleId === 4 ? "她" : "钦灵",
        ai_subtitle: "文学测试部",
        thinking_message: "",
        scale: 1.25,
        offset_x: 0,
        offset_y: 0,
        scale_p: 1,
        offset_x_p: 0,
        offset_y_p: 0,
        bubble_top: 5,
        bubble_left: 20,
        clothes: [],
        clothes_name: "default",
        character_folder: args.roleId === 4 ? "DeepSeek" : "钦灵",
        avatar_mode: "image",
        avatar_mode_p: "image",
        live2d: null,
        body_part: null,
      };
    if (cmd === "get_avatar_file") {
      if (!args.roleId) throw new Error("Avatar lookup lost role ownership");
      return `/__asset/${args.roleId === 4 ? "main" : "ql"}/${encodeURIComponent(args.emotion)}.webp`;
    }
    if (cmd === "get_env_config_by_key") return { value: "false" };
    if (cmd === "get_env_config") return [];
    if (cmd === "get_script_cursor_position") return { x: 750, y: 400 };
    if (cmd.startsWith("plugin:window|")) {
      if (cmd.endsWith("scale_factor")) return 1;
      if (cmd.endsWith("is_focused")) return false;
      return { width: 1500, height: 800, x: 0, y: 0 };
    }
    if (/^(music_|ambient_|get_.*list|list_)/.test(cmd)) return [];
    return null;
  },
  { shouldMockEvents: true },
);
window.__TAURI_INTERNALS__.convertFileSrc = (path: string) => path;
localStorage.setItem("lingchat_loading_shown", "1");

const { createApp, h, nextTick } = await import("vue");
const { createPinia } = await import("pinia");
const { createMemoryHistory, createRouter } = await import("vue-router");
const { i18n } = await import("@lingchat/host/locales");
const { default: MainChat } = await import("@lingchat/host/components/views/MainChat.vue");
const { initializeEventProcessors } = await import("@lingchat/host/core/events");
const { initializeTauriEventListeners } = await import("@lingchat/host/api/tauri-events");
const { eventQueue } = await import("@lingchat/host/core/events/event-queue");
const { eventProcessorManager } = await import("@lingchat/host/core/events/event-processor");
const { useGameStore } = await import("@lingchat/host/stores/modules/game");
const { useUIStore } = await import("@lingchat/host/stores/modules/ui/ui");
const { useSettingsStore } = await import("@lingchat/host/stores/modules/settings");
const { emit } = await import("@tauri-apps/api/event");
await import("@lingchat/host/assets/styles/base.css");
await import("@lingchat/host/assets/styles/variables.css");
await import("@lingchat/host/assets/styles/dialogue-text.css");

const app = createApp({ render: () => h(MainChat) });
app.use(createPinia());
app.use(i18n);
const router = createRouter({
  history: createMemoryHistory(),
  routes: [{ path: "/", component: MainChat }],
});
app.use(router);
await router.isReady();
const game = useGameStore(),
  ui = useUIStore(),
  settings = useSettingsStore();
game.initialized = true;
game.mainRoleId = 4;
game.currentInteractRoleId = 4;
game.userName = "测试玩家";
game.userSubtitle = "";
settings.setCurrentBackground("/__asset/bg/白天.webp");
ui.aspectRatio = 1500 / 800;
ui.autoMode = false;
ui.typeWriterSpeed = 2;
ui.characterVolume = 0;
ui.bubbleVolume = 0;
await initializeEventProcessors();
initializeTauriEventListeners();
game.enterStoryMode("第七个测试剧本", "horror", "plugins/seventh-test-script/seventh-test-script");
app.mount("#app");
await nextTick();
settings.setCurrentBackground("/__asset/bg/白天.webp");
window.__audit = {
  game,
  ui,
  settings,
  eventQueue,
  eventProcessorManager,
  emit,
  calls,
};
