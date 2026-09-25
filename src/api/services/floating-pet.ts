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
 * | Android | **本模块**：`SYSTEM_ALERT_WINDOW` 悬浮窗 + **搬运主 WebView** |
 * | iOS | 不支持（系统不允许跨 App 覆盖窗口） |
 *
 * 调用方应先用 {@link getFloatingPetStatus} 探测，再决定走哪条路径。
 *
 * ## 关键：搬运而非新建
 *
 * 桌面端的桌宠**不是新窗口**——它把 `main` 窗口改属性（去边框、缩尺寸、
 * 置顶），于是同一个 WebView 从「聊天界面」变成「桌宠」，IPC 与 store
 * 全部原样保留。
 *
 * Android 侧现在采用同样的思路：原生把**主 WebView 本身**从 Activity
 * 视图树搬进悬浮窗，同时给 Activity 塞一个占位页避免白屏。
 * 因此：
 *
 * - 悬浮窗里的页面**就是主界面**，`invoke()` 可用、store 数据完整
 * - 不需要任何 JS 桥或数据镜像
 * - 页面通过 {@link isInFloatingWindow} 感知自己已被搬入悬浮窗
 *
 * 进入顺序**必须先切页再搬移**，否则用户会看到主界面闪一下才变成桌宠。
 */

import { invoke } from "@tauri-apps/api/core";

const PLUGIN = "plugin:floating-pet";

/** 平台能力与授权状态的聚合结果。 */
export interface FloatingPetStatus {
  /** 当前平台是否支持系统级悬浮窗。 */
  supported: boolean;
  /** 是否已获得「显示在其他应用上层」权限。 */
  granted: boolean;
  /** 悬浮窗当前是否可见（WebView 已被搬入悬浮窗）。 */
  visible: boolean;
}

/** 进入悬浮桌宠的流程结果。 */
export type EnterResult = "shown" | "need-permission" | "unsupported";

/**
 * 页面是否已被搬入悬浮窗。
 *
 * 由原生在搬移完成后通过 `evaluateJavascript` 设置（见 Kotlin 侧
 * `notifyWeb("pet-detached", ...)`）。用全局标记而非 Tauri IPC 查询，
 * 是为了让页面在**搬移发生的那一帧**就能切换布局，不必等一次异步往返。
 */
let detachedIntoOverlay = false;

/** 标记页面已进入/离开悬浮窗形态。由原生事件驱动。 */
export function markFloatingWindowMode(active: boolean): void {
  detachedIntoOverlay = active;
}

/**
 * 当前页面是否运行在悬浮窗里。
 *
 * 注意这里**不能**再用 `getCurrentWindow()` 抛错来判断：现在悬浮窗里
 * 就是主 WebView，IPC 完全可用，那个判据已经失效。
 */
export function isInFloatingWindow(): boolean {
  return detachedIntoOverlay;
}

/**
 * 监听原生发来的「已搬入/已移出悬浮窗」事件。
 *
 * 原生通过 `window.dispatchEvent(new CustomEvent(...))` 派发，
 * 因此这里用标准 DOM 事件监听，不占用 Tauri IPC。
 *
 * @returns 取消监听的函数。
 */
export function onFloatingWindowModeChange(handler: (active: boolean) => void): () => void {
  const onDetached = () => {
    markFloatingWindowMode(true);
    handler(true);
  };
  const onAttached = () => {
    markFloatingWindowMode(false);
    handler(false);
  };
  window.addEventListener("pet-detached", onDetached);
  window.addEventListener("pet-attached", onAttached);
  return () => {
    window.removeEventListener("pet-detached", onDetached);
    window.removeEventListener("pet-attached", onAttached);
  };
}

/**
 * 监听原生发来的「展开状态变更」事件。
 *
 * 展开/收起时窗口尺寸由原生改变（头像态 ≈ 1/6 屏宽，展开态 ≈ 2/5 屏宽），
 * 页面需要同步切换布局。
 */
export function onPetExpandedChange(handler: (expanded: boolean) => void): () => void {
  const listener = (e: Event) => {
    const detail = (e as CustomEvent<{ expanded: boolean }>).detail;
    handler(!!detail?.expanded);
  };
  window.addEventListener("pet-expanded-changed", listener);
  return () => window.removeEventListener("pet-expanded-changed", listener);
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
 * 把主 WebView 搬进悬浮窗。
 *
 * 窗口尺寸**不由前端指定**：手机端按屏幕宽度的比例在原生侧计算
 * （收起态 1/6 屏宽、展开态 2/5 屏宽），只有原生知道真实屏幕宽度。
 *
 * @throws 未授权时抛出 `PERMISSION_DENIED`。请优先使用 {@link enterFloatingPet}。
 */
export async function showFloatingPet(options?: {
  scale?: number;
  x?: number;
  y?: number;
}): Promise<void> {
  // 先把标记打上再 await：原生 addView 完成时页面必须已经是悬浮窗布局，
  // 否则会看到「主界面布局被塞进小窗」的闪动。
  markFloatingWindowMode(true);
  try {
    await invoke(`${PLUGIN}|show`, {
      args: {
        scale: options?.scale ?? 1,
        x: options?.x ?? 0,
        y: options?.y ?? 0,
      },
    });
  } catch (e) {
    // 搬移失败要回滚标记，否则页面会停在悬浮窗布局而实际仍在 Activity 里
    markFloatingWindowMode(false);
    throw e;
  }
}

/** 悬浮窗当前是否可见。 */
export async function isVisible(): Promise<boolean> {
  return invoke<boolean>(`${PLUGIN}|is_visible`);
}

/**
 * 收起桌宠：把 WebView 搬回 Activity，恢复主界面。
 *
 * 与桌面端 `set_pet_mode(enable=false)` 对应——那边恢复窗口属性，
 * 这边恢复视图父子关系。
 */
export async function hideFloatingPet(): Promise<void> {
  await invoke(`${PLUGIN}|hide`);
  markFloatingWindowMode(false);
}

/** 移动悬浮窗到指定坐标（dp）。 */
export async function moveFloatingPet(x: number, y: number): Promise<void> {
  await invoke(`${PLUGIN}|move_pet`, { args: { x, y } });
}

/** 更新悬浮窗尺寸（dp）。一般不需要——展开/收起请用 {@link setFloatingPetExpanded}。 */
export async function resizeFloatingPet(width: number, height: number): Promise<void> {
  await invoke(`${PLUGIN}|set_size`, { args: { width, height } });
}

/**
 * 展开 / 收起桌宠。
 *
 * 收起态只显示头像（约 1/6 屏宽），展开后容纳头像 + 输入框（约 2/5 屏宽）。
 * 原生会 `updateViewLayout` 改窗口尺寸并派发 `pet-expanded-changed` 事件。
 *
 * 手机端没有鼠标悬停，因此由「点击头像」触发——对应桌面端的
 * `mouseenter/mouseleave` 自动展开。
 */
export async function setFloatingPetExpanded(expanded: boolean): Promise<void> {
  await invoke(`${PLUGIN}|set_expanded`, { expanded });
}

/**
 * 切换悬浮窗点击穿透。
 *
 * - `false`：整个悬浮窗不接收触摸，手势落到下层 App（桌宠「挂机」状态）
 * - `true`：悬浮窗可交互（点击角色、输入消息）
 *
 * 注意这是窗口级开关，无法按区域精细控制，因此悬浮窗尺寸已按宠物本身
 * 收窄（头像态仅 1/6 屏宽），不做大块透明留白。
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
 * 进入悬浮桌宠的完整流程：探测能力 → 检查授权 → 必要时引导授权 → 搬运 WebView。
 *
 * **调用前必须已经把页面切到 `/pet` 路由**（`router.push("/pet")`），
 * 否则搬移的瞬间用户会看到主界面闪一下。切页与搬移的顺序由调用方保证，
 * 见 `MainChat.vue` 的 `goToPetMode`。
 *
 * @returns
 *  - `'shown'`：WebView 已搬入悬浮窗
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
