/**
 * tauri-plugin-floating-pet — 前端调用封装
 *
 * 为 LingChat 的桌宠模式提供 **Android 系统级悬浮窗**能力：
 * 一个可浮在其他 App 之上、可拖拽、可切换点击穿透的透明小窗，
 * 内部加载 `/pet` 路由复用现有的角色渲染逻辑。
 *
 * ## 平台差异
 *
 * | 平台 | 行为 |
 * |------|------|
 * | Android | 真·系统悬浮窗，可浮在其他 App 之上 |
 * | 桌面端 | 不支持，请走 `api::pet` 的原生窗口路径（`set_pet_mode`） |
 * | iOS | 不支持，系统不允许跨 App 覆盖窗口 |
 *
 * 使用前应先 `getStatus()` 探测，再决定走哪条路径。
 */

import { invoke } from '@tauri-apps/api/core';

/** `show()` 的参数。尺寸与坐标单位均为 **逻辑像素（dp）**。 */
export interface ShowOptions {
  /** 悬浮窗内加载的 URL，默认指向 LingChat 自身的 `/pet` 路由。 */
  url?: string;
  /** 悬浮窗宽度（dp）。会被限制在 [80, 720]。 */
  width: number;
  /** 悬浮窗高度（dp）。会被限制在 [80, 720]。 */
  height: number;
  /** 初始 X 坐标（dp），屏幕左上角为原点。 */
  x?: number;
  /** 初始 Y 坐标（dp），屏幕左上角为原点。 */
  y?: number;
}

/** 平台能力与授权状态的聚合结果。 */
export interface PetStatus {
  /** 当前平台是否支持系统级悬浮窗。 */
  supported: boolean;
  /** 是否已获得「显示在其他应用上层」权限。 */
  granted: boolean;
  /** 悬浮窗当前是否可见。 */
  visible: boolean;
}

/** 悬浮窗默认加载的页面地址。 */
const DEFAULT_PET_URL = 'http://tauri.localhost/pet';

const PLUGIN = 'plugin:floating-pet';

/** 当前平台是否支持系统级悬浮窗。 */
export async function isSupported(): Promise<boolean> {
  return invoke<boolean>(`${PLUGIN}|is_supported`);
}

/** 是否已获得「显示在其他应用上层」权限。 */
export async function checkPermission(): Promise<boolean> {
  return invoke<boolean>(`${PLUGIN}|check_permission`);
}

/**
 * 跳转到系统的悬浮窗授权页。
 *
 * 该权限无法通过运行时弹窗申请，只能引导用户手动开启。
 * 用户在设置页操作后**不会收到回调**，需在 App 恢复前台时
 * 重新 `checkPermission()` 确认结果。
 */
export async function requestPermission(): Promise<void> {
  await invoke(`${PLUGIN}|request_permission`);
}

/** 一次性获取平台能力与授权状态。 */
export async function getStatus(): Promise<PetStatus> {
  return invoke<PetStatus>(`${PLUGIN}|status`);
}

/**
 * 显示悬浮窗。
 *
 * 幂等：若已显示会先隐藏再重建，避免原生侧窗口泄漏。
 *
 * @throws 未授权时抛出 `PERMISSION_DENIED`；平台不支持时抛出错误。
 */
export async function show(options: ShowOptions): Promise<void> {
  await invoke(`${PLUGIN}|show`, {
    args: {
      url: options.url ?? DEFAULT_PET_URL,
      width: options.width,
      height: options.height,
      x: options.x ?? 0,
      y: options.y ?? 0,
    },
  });
}

/** 隐藏悬浮窗。未显示时安全返回。 */
export async function hide(): Promise<void> {
  await invoke(`${PLUGIN}|hide`);
}

/** 移动悬浮窗到指定坐标（dp）。 */
export async function movePet(x: number, y: number): Promise<void> {
  await invoke(`${PLUGIN}|move_pet`, { args: { x, y } });
}

/** 调整悬浮窗尺寸（dp）。 */
export async function setSize(width: number, height: number): Promise<void> {
  await invoke(`${PLUGIN}|set_size`, { args: { width, height } });
}

/**
 * 切换点击穿透。
 *
 * - `touchable = false`：整个悬浮窗不接收触摸，手势落到下层 App
 * - `touchable = true`：悬浮窗可交互（输入框可正常弹键盘）
 *
 * 注意这是**窗口级开关**，无法按区域精细控制，
 * 因此悬浮窗尺寸应紧贴角色，避免大块透明留白。
 */
export async function setTouchable(touchable: boolean): Promise<void> {
  await invoke(`${PLUGIN}|set_touchable`, { touchable });
}

/** 悬浮窗当前是否可见。 */
export async function isVisible(): Promise<boolean> {
  return invoke<boolean>(`${PLUGIN}|is_visible`);
}

/**
 * 完整的「进入悬浮桌宠」流程：探测能力 → 检查授权 → 必要时引导授权 → 显示。
 *
 * @returns `'shown'` 成功显示；`'need-permission'` 已引导授权，等待用户返回后重试；
 *          `'unsupported'` 当前平台不支持。
 */
export async function enterFloatingPet(
  options: ShowOptions
): Promise<'shown' | 'need-permission' | 'unsupported'> {
  if (!(await isSupported())) return 'unsupported';

  if (!(await checkPermission())) {
    await requestPermission();
    return 'need-permission';
  }

  await show(options);
  return 'shown';
}
