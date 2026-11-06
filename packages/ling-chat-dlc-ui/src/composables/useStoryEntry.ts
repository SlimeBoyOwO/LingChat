import { ref, onBeforeUnmount } from "vue";
import { useRouter } from "vue-router";
import { type ScriptSummary, startScript } from "@lingchat/host/api/services/script-info";
import { resetScriptState, checkScriptGhostLock } from "@lingchat/dlc-ui/api/services/story";
import { useGameStore } from "@lingchat/host/stores/modules/game";
import { useDialogStore } from "@lingchat/host/stores/modules/ui/dialog";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
import { i18n } from "@lingchat/host/locales";
import { eventQueue } from "@lingchat/host/core/events/event-queue";
export function useStoryEntry(emit: (event: "script-state-reset") => void) {
  const router = useRouter();
  const gameStore = useGameStore();
  const dialogStore = useDialogStore();
  const uiStore = useUIStore();
  const resettingName = ref<string | null>(null);
  const resetDoneName = ref<string | null>(null);
  let resetDoneTimer = 0;

  const resetMemory = async (script: ScriptSummary) => {
    if (resettingName.value) return;
    const confirmed = await dialogStore.confirm(
      i18n.global.t("views.menu.resetMemoryMessage", {
        name: script.script_name,
      }),
      i18n.global.t("views.menu.resetMemoryTitle"),
    );
    if (!confirmed) return;

    resettingName.value = script.script_name;
    try {
      const removed = await resetScriptState(script.script_name);
      emit("script-state-reset");
      await dialogStore.alert(
        removed
          ? i18n.global.t("views.menu.resetMemoryDone")
          : i18n.global.t("views.menu.resetMemoryEmpty"),
        i18n.global.t("views.menu.resetMemoryTitle"),
      );
      resetDoneName.value = script.script_name;
      clearTimeout(resetDoneTimer);
      resetDoneTimer = window.setTimeout(() => (resetDoneName.value = null), 2000);
    } catch {
      await dialogStore.alert(
        i18n.global.t("views.menu.resetMemoryFailed"),
        i18n.global.t("views.menu.resetMemoryTitle"),
      );
    } finally {
      resettingName.value = null;
    }
  };

  const selectScript = async (script: ScriptSummary) => {
    // 删角色文件彩蛋（DDLC ghost menu 对应物）：该剧本的 .chr 被玩家全部删掉后，
    // 进入不再走正常流程——锁成纯黑底 + 黑白幽灵立绘，没有任何出口按钮，
    // 只有玩家自己放回 .chr（轮询自动解锁）或点窗口 X 放大脸退出两条路。
    // 实时查询而非读列表缓存：玩家可能刚在另一个窗口删完或放回文件。
    const ghostLock = await checkScriptGhostLock(script.script_name);
    if (ghostLock.entry_error) {
      await dialogStore.alert(ghostLock.entry_error, "剧本加载失败");
      return;
    }
    if (ghostLock.locked && ghostLock.asset_dir) {
      uiStore.openGhostLock(script.script_name, ghostLock.asset_dir);
      return;
    }

    if (!(await confirmStoryEntry(script))) return;

    // 新剧本必须使用全新的队列代号，取消任何上轮尚未返回的点击/视觉计时。
    eventQueue.clear();
    // 先标记剧情模式再挂载 MainChat：初始化逻辑据此跳过自由对话入场问候，
    // 防止后台生成的寒暄在剧本开始后才插入队列并拖慢剧情。
    gameStore.enterStoryMode(script.script_name, script.content_warning, script.folder_key);
    try {
      await router.push("/chat");
      await startScript(script.script_name);
    } catch (error) {
      gameStore.exitStoryMode();
      await router.push("/");
      await dialogStore.alert(String(error), "剧本加载失败");
    }
  };

  onBeforeUnmount(() => clearTimeout(resetDoneTimer));
  return { resettingName, resetDoneName, resetMemory, selectScript };
}

export async function confirmStoryEntry(script: ScriptSummary): Promise<boolean> {
  const dialogStore = useDialogStore(),
    uiStore = useUIStore();
  // 带内容警告的剧本（如恐怖向）先弹确认，取消则不进入
  if (script.content_warning === "horror") {
    const confirmed = await dialogStore.confirm(
      i18n.global.t("views.contentWarning.horrorMessage"),
      i18n.global.t("views.contentWarning.horrorTitle"),
    );
    if (!confirmed) return false;

    // 确认后先"卡死 → 花屏"再进入（恐怖演出的一部分）
    await uiStore.beginHorrorEntry();
  }

  return true;
}
