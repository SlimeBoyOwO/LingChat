/**
 * 应用启动初始化 Composable
 *
 * 集中处理挂载时的一次性初始化：
 *   - UI Store（加载角色 tips）
 *   - 自动弹出独立日志窗口（仅主窗口触发，开关在日志页设置）
 *   - LLM 提供商配置预加载，避免主界面因 store 未加载而误判未选择模型
 *   - 成就系统的 window 控制台测试钩子 + WebSocket 解锁监听
 *
 * 在 App.vue 中调用一次。
 */
import { onMounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRouter } from "vue-router";
import { initUIStore } from "@/stores/modules/ui/ui";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useGameStore } from "@/stores/modules/game";
import { useSettingsStore } from "@/stores/modules/settings";
import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
import { useAchievementStore } from "@/stores/modules/ui/achievement";
import { eventQueue } from "@/core/events/event-queue";
import { getAutostartStatus } from "@/api/services/config";

async function waitEntryGreetingDone(trigger: () => Promise<unknown>) {
  let settled = false;
  let resolveDone!: () => void;
  let unlisten: (() => void) | null = null;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const done = new Promise<void>((resolve) => {
    resolveDone = resolve;
  });
  const finish = () => {
    if (settled) return;
    settled = true;
    if (timeout) clearTimeout(timeout);
    unlisten?.();
    unlisten = null;
    resolveDone();
  };

  const registeredUnlisten = await listen("entry:greeting-done", finish).catch((err) => {
    console.warn("[Entry] 完成事件监听注册失败（非致命）:", err);
    return null;
  });
  if (settled) registeredUnlisten?.();
  else unlisten = registeredUnlisten;
  timeout = setTimeout(finish, 15000);

  try {
    void trigger();
    if (!registeredUnlisten) finish();
    await done;
  } finally {
    if (timeout) clearTimeout(timeout);
    unlisten?.();
  }
}

export function useAppBootstrap() {
  const router = useRouter();
  const uiStore = useUIStore();
  const gameStore = useGameStore();
  const settingsStore = useSettingsStore();

  onMounted(() => {
    // 初始化 UI Store（加载角色 tips）
    initUIStore();

    if (getCurrentWindow().label === "main") {
      void getAutostartStatus()
        .then(async (auto) => {
          if (auto.auto_play) uiStore.autoMode = true;
          const wantPet =
            (auto.launched_by_autostart && auto.boot_as_pet) ||
            (!auto.launched_by_autostart && auto.startup_pet_mode);

          if (!wantPet) {
            if (auto.auto_start_tts) {
              invoke("autostart_boot_apply", { roleId: null }).catch((e) =>
                console.warn("[Autostart] 正常启动拉起 TTS 失败（非致命）:", e),
              );
            }
            return;
          }

          const petRoleId = Number(auto.pet_role_id) || 0;
          uiStore.petReady = false;
          uiStore.petBooting = true;
          let petBootSucceeded = false;
          try {
            await invoke("set_pet_mode", {
              enable: true,
              scale: settingsStore.pet.scale || 1.0,
            }).catch((err) => {
              console.warn("[Autostart] 提前进入桌宠置顶状态失败（非致命）:", err);
            });

            await gameStore.bootAsPet(petRoleId > 0 ? petRoleId : undefined);
            const roleId =
              gameStore.mainRoleId > 0 ? gameStore.mainRoleId : petRoleId > 0 ? petRoleId : null;
            router.push("/pet");

            const ttsReady = auto.auto_start_tts
              ? invoke("autostart_boot_apply", { roleId }).catch(() => {})
              : Promise.resolve();
            await Promise.all([useLlmProvidersStore().load().catch(() => {}), ttsReady]);

            if (auto.startup_greeting) {
              await waitEntryGreetingDone(() =>
                invoke("notify_player_entry").catch((err) => {
                  console.warn("[Entry] 问候触发失败（非致命）:", err);
                }),
              );
            }

            await new Promise((resolve) => setTimeout(resolve, 2000));
            petBootSucceeded = true;
          } catch (e) {
            console.error("[Autostart] 启动即桌宠初始化失败:", e);
            await invoke("update_solid_regions", { rects: [] }).catch(() => {});
            await invoke("set_pet_mode", { enable: false }).catch(() => {});
            await router.replace("/chat").catch(() => {});
          } finally {
            eventQueue.resume();
            uiStore.petReady = petBootSucceeded;
            uiStore.petBooting = false;
          }
        })
        .catch((e) => console.error("[Autostart] 读取启动配置失败:", e));
    }

    // 启动时自动弹出独立日志窗口（仅主窗口触发，开关在日志页设置）
    if (
      getCurrentWindow().label === "main" &&
      localStorage.getItem("lingchat_log_window_auto_open") === "1"
    ) {
      invoke("open_log_window").catch((e) => console.error("自动打开日志窗口失败:", e));
    }

    // 预加载 LLM 提供商配置，避免主界面因 store 未加载而误判未选择模型
    const llmStore = useLlmProvidersStore();
    llmStore.load().catch((e) => console.error("加载 LLM 提供商失败:", e));

    // 供成就系统控制台测试用，在 window 对象中注册一些方法
    const achievementStore = useAchievementStore();
    (window as any).requestAchievementUnlock = (data: any) =>
      achievementStore.notifyBackendUnlock(data);
    (window as any).showAchievement = (data: any) => achievementStore.addAchievement(data);
    // 成就系统启动WebSocket监听
    achievementStore.listenForUnlocks();
  });
}
