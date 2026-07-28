/**
 * 悬浮桌宠 UI Store（仅 Android 端生效）。
 *
 * 职责：
 * - 跟踪启用开关 / 权限状态 / 菜单可见性；
 * - 提供 start / hide / stop 三个动作统一处理事件；
 * - 把 WebView 侧的桌宠状态推送到 Android Service（throttled）。
 */
import { defineStore } from "pinia";
import {
  checkOverlayPermission,
  hideFloatingPet,
  markPermissionExplanationShown,
  onPetEvent,
  requestOverlayPermission,
  showFloatingPet,
  startPermissionExplanation,
  stopFloatingPetService,
  updatePetState,
  type OverlayPermissionStatus,
  type PetEvent,
  type PetStatePayload,
} from "@/api/services/floating-pet";

const PUSH_THROTTLE_MS = 200;

export type FloatingPetMenuKind = "hidden" | "settings" | "exit-confirm";

interface FloatingPetState {
  /** 总开关（设置页控制）。 */
  enabled: boolean;
  /** 权限状态。 */
  permission: OverlayPermissionStatus;
  /** 是否已展示过一次性权限说明。 */
  explanationShown: boolean;
  /** 当前菜单类型。 */
  menu: FloatingPetMenuKind;
  /** 上一次手势事件（用于调试）。 */
  lastEvent: PetEvent | null;
  /** 当前推送中的状态（供设置页预览）。 */
  lastPushed: PetStatePayload | null;
}

export const useFloatingPetStore = defineStore("floating-pet", {
  state: (): FloatingPetState => ({
    enabled: false,
    permission: "unknown",
    explanationShown: false,
    menu: "hidden",
    lastEvent: null,
    lastPushed: null,
  }),

  getters: {
    isReady(state): boolean {
      return (
        state.enabled &&
        state.permission === "granted" &&
        state.menu !== "exit-confirm"
      );
    },
    isSupported(state): boolean {
      return state.permission !== "unsupported";
    },
  },

  actions: {
    /**
     * 进入聊天界面或桌宠按钮被点击时调用。
     * - 未开启：返回 false（前端不要调用 show_floating_pet）
     * - 权限缺失：尝试拉起一次性说明
     * - 权限通过：调 showFloatingPet
     */
    async activate(scale?: number): Promise<boolean> {
      if (!this.enabled) return false;
      const status = await checkOverlayPermission();
      this.permission = status;
      if (status === "unsupported") {
        // 桌面 / iOS 上保持开启但不可用
        return false;
      }
      if (status !== "granted") {
        if (!this.explanationShown) {
          await startPermissionExplanation();
          this.explanationShown = true;
          await markPermissionExplanationShown();
        }
        await requestOverlayPermission();
        return false;
      }
      await showFloatingPet(scale);
      return true;
    },

    async deactivate(): Promise<void> {
      await hideFloatingPet();
      this.menu = "hidden";
    },

    async stop(): Promise<void> {
      await stopFloatingPetService();
      this.menu = "hidden";
      this.lastPushed = null;
    },

    async refreshPermission(): Promise<OverlayPermissionStatus> {
      this.permission = await checkOverlayPermission();
      return this.permission;
    },

    setEnabled(v: boolean) {
      this.enabled = v;
      if (!v) void this.deactivate();
    },

    showMenu(kind: Exclude<FloatingPetMenuKind, "hidden">) {
      this.menu = kind;
    },

    hideMenu() {
      this.menu = "hidden";
    },

    pushState(payload: PetStatePayload) {
      this.lastPushed = payload;
      void updatePetState(payload);
    },

    /**
     * 订阅 Android 端手势事件。建议在 app 启动时调用一次。
     * 返回取消订阅函数。
     */
    bindEventBus(): () => void {
      return onPetEvent((event) => {
        this.lastEvent = event;
        if (event.type === "double_tap") {
          void this.deactivate();
        } else if (event.type === "long_press") {
          this.menu = "settings";
        }
      });
    },
  },
});

/**
 * 节流推送 helper。组件层使用：避免每个 reactive 字段变化都触发 invoke。
 */
let pending: PetStatePayload | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;

export function useFloatingPetPusher() {
  const store = useFloatingPetStore();
  return (next: PetStatePayload) => {
    pending = { ...pending, ...next };
    if (timer) return;
    timer = setTimeout(() => {
      if (pending) store.pushState(pending);
      pending = null;
      timer = null;
    }, PUSH_THROTTLE_MS);
  };
}
