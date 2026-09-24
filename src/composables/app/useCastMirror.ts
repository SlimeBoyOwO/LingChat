/**
 * 投屏镜像 Composable
 *
 * 投屏窗口是独立的 webview，事件队列与主窗口各自独立；主窗口靠玩家点击
 * 逐句推进，投屏窗口若自己消费 ai:reply 就会「按消息到达时间显示」而脱节。
 * 这里在台词变化时把主窗口正在显示的完整状态（台词/标题/情绪/背景/场景/
 * 角色）经 Rust 中继 cast_emit_mirror 存储并广播 cast:mirror，投屏窗口直接
 * 应用——主窗口显示什么，投屏就显示什么。只在主窗口上报：投屏窗口自己也
 * 会改 showCharacterLine（应用镜像时），绝不能把投屏的状态回写给主窗口。
 *
 * 在 App.vue 中调用一次以激活。
 */
import { watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useGameStore } from "@/stores/modules/game";

export function useCastMirror() {
  const uiStore = useUIStore();
  const gameStore = useGameStore();
  // 只主窗口上报镜像：投屏窗口应用镜像时也会改 showCharacterLine / 背景，
  // 绝不能把投屏窗口的状态回写给主窗口造成环路。
  const isMainWindow = getCurrentWindow().label === "main";

  function sendMirror() {
    if (!isMainWindow) return;
    invoke("cast_emit_mirror", {
      mirror: {
        line: uiStore.showCharacterLine,
        title: uiStore.showCharacterTitle,
        subtitle: uiStore.showCharacterSubtitle,
        emotion: uiStore.showCharacterEmotion,
        motionText: uiStore.showCharacterMotionText,
        status: gameStore.currentStatus,
        background: uiStore.currentBackground,
        backgroundEffect: uiStore.currentBackgroundEffect,
        currentSceneId: gameStore.currentScene?.id ?? null,
        presentRoleIds: [...gameStore.presentRoleIds],
        currentRoleId: gameStore.currentInteractRoleId,
        // 当前交互角色的情绪状态：驱动投屏窗口的 Live2D 表情 / 静态立绘切换。
        // showCharacterEmotion 只是对话框里的文本标签（= originalEmotion），真正用来切
        // 表情的是角色对象的 emotion / originalEmotion；投屏窗口不消费 ai:reply
        // （否则按消息到达时间显示），只能靠镜像在这里把角色状态一起带过去。
        roleEmotion: gameStore.currentInteractRole?.emotion ?? "",
        roleOriginalEmotion: gameStore.currentInteractRole?.originalEmotion ?? "",
      },
    }).catch((e) => console.warn("[Cast] 上报投屏镜像失败：", e));
  }

  watch(() => uiStore.showCharacterLine, sendMirror);
  // 背景图 / 背景特效变化同样广播镜像：背景事件与台词事件相互独立，
  // 只监听台词的话，换背景 / 换特效时投屏窗口收不到任何更新。
  watch(() => uiStore.currentBackground, sendMirror);
  watch(() => uiStore.currentBackgroundEffect, sendMirror);
}
