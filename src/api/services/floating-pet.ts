import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Android 悬浮叠加层权限状态（Rust 端 OverlayPermissionStatus 镜像）。 */
export type OverlayPermissionStatus =
  | "granted"
  | "denied"
  | "unknown"
  | "unsupported";

/** 桌宠可见性 + 渲染参数。 */
export interface PetStatePayload {
  character?: {
    id: string;
    name: string;
    avatarUrl: string;
    expression: string;
  };
  dialogue?: {
    text: string;
    isTyping: boolean;
    audioPlaying: boolean;
  };
  scale?: number; // 0.5 - 2.0
  volume?: number; // 0 - 100
  backgroundEffect?: string;
  visible?: boolean;
}

/** 手势事件。 */
export interface PetEvent {
  type: "tap" | "double_tap" | "long_press" | "drag_end" | "pinch";
  payload?: { x?: number; y?: number; scale?: number };
}

export type PetEventHandler = (e: PetEvent) => void;

const EVENT_NAME = "floating-pet://event";

let eventUnlisten: UnlistenFn | null = null;
const eventListeners = new Set<PetEventHandler>();
let eventSubscription: Promise<void> | null = null;

async function ensureEventSubscription(): Promise<void> {
  if (eventUnlisten) return;
  if (!eventSubscription) {
    eventSubscription = listen<PetEvent>(EVENT_NAME, (event) => {
      for (const h of eventListeners) h(event.payload);
    })
      .then((unlisten) => {
        eventUnlisten = unlisten;
        if (eventListeners.size === 0) {
          unlisten();
          eventUnlisten = null;
        }
      })
      .finally(() => {
        eventSubscription = null;
      });
  }
  await eventSubscription;
}

/**
 * 订阅桌宠手势事件。返回取消订阅函数。
 *
 * 多次调用只会在内部维护一份 Tauri listener；仅在全部取消时清理。
 */
export function onPetEvent(handler: PetEventHandler): () => void {
  void ensureEventSubscription();
  eventListeners.add(handler);
  return () => {
    eventListeners.delete(handler);
    if (eventListeners.size === 0 && eventUnlisten) {
      eventUnlisten();
      eventUnlisten = null;
    }
  };
}

export async function checkOverlayPermission(): Promise<OverlayPermissionStatus> {
  try {
    return await invoke<OverlayPermissionStatus>("plugin:floating-pet|check_overlay_permission");
  } catch {
    return "unsupported";
  }
}

export async function requestOverlayPermission(): Promise<void> {
  await invoke("plugin:floating-pet|request_overlay_permission");
}

export async function showFloatingPet(scale?: number): Promise<void> {
  await invoke("plugin:floating-pet|show_floating_pet", { scale });
}

export async function hideFloatingPet(): Promise<void> {
  await invoke("plugin:floating-pet|hide_floating_pet");
}

export async function stopFloatingPetService(): Promise<void> {
  await invoke("plugin:floating-pet|stop_floating_pet_service");
}

export async function stopFloatingPetServiceWithConfirmation(): Promise<boolean> {
  return await invoke<boolean>("plugin:floating-pet|stop_floating_pet_service_with_confirmation");
}

export async function updatePetState(payload: PetStatePayload): Promise<void> {
  await invoke("plugin:floating-pet|update_pet_state", { payload });
}

export async function startPermissionExplanation(): Promise<void> {
  await invoke("plugin:floating-pet|start_permission_explanation");
}

export async function markPermissionExplanationShown(): Promise<void> {
  await invoke("plugin:floating-pet|mark_permission_explanation_shown");
}
