/**
 * 桌宠悬浮窗（Android 系统级悬浮窗）前端调用封装。
 *
 * 对应 Rust 插件 `tauri-plugin-floating-pet`（见 `src-tauri/plugins/tauri-plugin-floating-pet`）。
 *
 * ## 平台差异
 *
 * | 平台 | 桌宠实现 |
 * |------|----------|
 * | 桌面端 | `api::pet` 的原生窗口（`set_pet_mode`），透明 + 置顶 + 点击穿透 |
 * | Android | **本模块**：`SYSTEM_ALERT_WINDOW` 系统悬浮窗，可浮在其他 App 之上 |
 * | iOS | 不支持（系统不允许跨 App 覆盖窗口） |
 *
 * 调用方应先用 {@link getFloatingPetStatus} 探测，再决定走哪条路径。
 */

import { invoke } from "@tauri-apps/api/core";
import {
  AVATAR_BAND_BASE,
  CHAT_BASE_H,
  DIALOG_MAX_BASE,
  PET_WIDTH_BASE,
} from "@/components/pet/constants";

const PLUGIN = "plugin:floating-pet";

/** 悬浮窗默认加载的页面地址（App 自身的 /pet 路由）。 */
const PET_ROUTE = "/pet";

/** 尺寸限制（dp），与 Kotlin 侧 MIN_SIZE_DP / MAX_SIZE_DP 保持一致。 */
const MIN_SIZE_DP = 80;
const MAX_SIZE_DP = 720;

/** 平台能力与授权状态的聚合结果。 */
export interface FloatingPetStatus {
  /** 当前平台是否支持系统级悬浮窗。 */
  supported: boolean;
  /** 是否已获得「显示在其他应用上层」权限。 */
  granted: boolean;
  /** 悬浮窗当前是否可见。 */
  visible: boolean;
}

/** 进入悬浮桌宠的流程结果。 */
export type EnterResult = "shown" | "need-permission" | "unsupported";

/**
 * 构造悬浮窗内的页面 URL。
 *
 * Android WebView 在 Tauri 中通过 `http://tauri.localhost` 访问内嵌前端资源，
 * 与桌面端一致，因此可以直接复用 `/pet` 路由与全部现有组件。
 */
function buildPetUrl(): string {
  return `${window.location.origin}${PET_ROUTE}`;
}

/**
 * 计算桌宠悬浮窗尺寸（dp）。
 *
 * 复用 `components/pet/constants.ts` 的基准值——它们同时也是桌面端窗口
 * 尺寸的计算依据（见 `src-tauri/src/api/pet.rs`），保证两端视觉一致。
 */
export function calcFloatingPetSize(scale = 1) {
  const s = Math.max(0.5, Math.min(scale, 3));
  const width = Math.round(PET_WIDTH_BASE * s);
  const height = Math.round((AVATAR_BAND_BASE + CHAT_BASE_H + DIALOG_MAX_BASE) * s);
  return {
    width: Math.max(MIN_SIZE_DP, Math.min(width, MAX_SIZE_DP)),
    height: Math.max(MIN_SIZE_DP, Math.min(height, MAX_SIZE_DP)),
  };
}

/** 查询平台能力与授权状态。任一平台均可安全调用。 */
export async function getFloatingPetStatus(): Promise<FloatingPetStatus> {
  try {
    return await invoke<FloatingPetStatus>(`${PLUGIN}|status`);
  } catch {
    // 插件不可用（如桌面端未注册、旧版本）时降级为「不支持」
    return { supported: false, granted: false, visible: false };
  }
}

/** 当前平台是否支持系统级悬浮窗。 */
export async function isFloatingPetSupported(): Promise<boolean> {
  return (await getFloatingPetStatus()).supported;
}

/**
 * 显示桌宠悬浮窗。
 *
 * @throws 未授权时抛出 `PERMISSION_DENIED`。请优先使用 {@link enterFloatingPet}，
 *         它会自动处理授权引导。
 */
export async function showFloatingPet(options?: {
  scale?: number;
  x?: number;
  y?: number;
}): Promise<void> {
  const { width, height } = calcFloatingPetSize(options?.scale ?? 1);
  await invoke(`${PLUGIN}|show`, {
    args: {
      url: buildPetUrl(),
      width,
      height,
      x: options?.x ?? 0,
      y: options?.y ?? 0,
    },
  });
}

/** 悬浮窗当前是否可见。 */
export async function isVisible(): Promise<boolean> {
  return invoke<boolean>(`${PLUGIN}|is_visible`);
}

/**
 * 悬浮窗 WebView 内注入的原生桥（见 Kotlin 侧 `PetBridge`）。
 *
 * 悬浮窗里没有 Tauri IPC，这是页面**唯一**能与原生通信的通道。
 * 目前只提供 `close()`：让悬浮窗能关闭自己，避免弹出后收不回去。
 */
interface PetBridge {
  close(): void;
}

declare global {
  interface Window {
    LingChatPet?: PetBridge;
  }
}

/** 当前页面是否运行在悬浮窗内（即存在原生桥）。 */
export function isInFloatingWindow(): boolean {
  return typeof window !== "undefined" && !!window.LingChatPet;
}

/**
 * 关闭当前悬浮窗。
 *
 * 与 {@link hideFloatingPet} 的区别：后者由**主界面**调用（走 Tauri IPC），
 * 本函数由**悬浮窗内部**调用（走原生注入桥）。悬浮窗里没有 IPC，必须用它。
 *
 * @returns 是否成功发起关闭。
 */
export function closeFloatingWindowFromInside(): boolean {
  const bridge = typeof window !== "undefined" ? window.LingChatPet : undefined;
  if (!bridge) return false;
  try {
    bridge.close();
    return true;
  } catch (e) {
    console.error("[floating-pet] 关闭悬浮窗失败:", e);
    return false;
  }
}

/** 隐藏桌宠悬浮窗（由主界面调用）。未显示时安全返回。 */
export async function hideFloatingPet(): Promise<void> {
  await invoke(`${PLUGIN}|hide`);
}

/** 移动悬浮窗到指定坐标（dp）。 */
export async function moveFloatingPet(x: number, y: number): Promise<void> {
  await invoke(`${PLUGIN}|move_pet`, { args: { x, y } });
}

/** 更新悬浮窗尺寸（跟随 pet.scale 设置）。 */
export async function resizeFloatingPet(scale: number): Promise<void> {
  const { width, height } = calcFloatingPetSize(scale);
  await invoke(`${PLUGIN}|set_size`, { args: { width, height } });
}

/**
 * 切换悬浮窗点击穿透。
 *
 * - `false`：整个悬浮窗不接收触摸，手势落到下层 App（桌宠「挂机」状态）
 * - `true`：悬浮窗可交互（点击角色、输入消息）
 *
 * 注意这是窗口级开关，无法按区域精细控制，因此悬浮窗尺寸已按宠物本身
 * 收窄（见 {@link calcFloatingPetSize}），不做大块透明留白。
 */
export async function setFloatingPetTouchable(touchable: boolean): Promise<void> {
  await invoke(`${PLUGIN}|set_touchable`, { touchable });
}

/**
 * 跳转到系统的「显示在其他应用上层」授权页。
 *
 * 该权限是 Android 特殊权限，无法运行时弹窗申请。用户操作后**不会收到回调**，
 * 需在 App 恢复前台时重新查询状态确认。
 */
export async function requestFloatingPetPermission(): Promise<void> {
  await invoke(`${PLUGIN}|request_permission`);
}

/**
 * 进入悬浮桌宠的完整流程：探测能力 → 检查授权 → 必要时引导授权 → 显示。
 *
 * @returns
 *  - `'shown'`：已成功显示
 *  - `'need-permission'`：已跳转授权页，用户返回后应重新调用本函数
 *  - `'unsupported'`：当前平台不支持（桌面端请走 `set_pet_mode` 路径）
 */
export async function enterFloatingPet(options?: {
  scale?: number;
  x?: number;
  y?: number;
}): Promise<EnterResult> {
  const status = await getFloatingPetStatus();

  if (!status.supported) return "unsupported";

  if (!status.granted) {
    await requestFloatingPetPermission();
    return "need-permission";
  }

  await showFloatingPet(options);
  return "shown";
}
