export interface Live2dMotionBinding {
  group: string;
  index: number;
  loop?: boolean;
  [key: string]: unknown;
}

export interface Live2dParameterBinding {
  parameter: string;
  gain?: number;
  [key: string]: unknown;
}

export interface Live2dEyeBlinkBinding {
  left: string;
  right: string;
  [key: string]: unknown;
}

export interface Live2dFocusAnchor {
  x: number;
  y: number;
  [key: string]: unknown;
}

/** 抚摸命中区域：相对 drawable bounds 的比例矩形，原点在左上、y 向下。 */
export interface Live2dTouchRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  [key: string]: unknown;
}

/** 抚摸反应绑定：某个部位被抚摸时先随手的移动方向晃动、换上一张表情，松手回正之后
    播放这里绑定的动作。动作与表情都可省略，两个都不给就只剩晃动。 */
export interface Live2dTouchBinding {
  group?: string;
  index?: number;
  loop?: boolean;
  /** 抚摸时换上的表情，直接给模型的表情名。省略表示不动表情。
      松手后（动作播完）会换回当前情绪的表情。 */
  expression?: string | null;
  /** 可选，覆盖由 focus_anchor 推出的默认区域。与默认模板同坐标系，
      所以可以只覆盖其中一部分位。 */
  region?: Live2dTouchRegion | null;
  [key: string]: unknown;
}

export interface Live2dVariant {
  model: string;
  default_expression?: string | null;
  expressions: Record<string, string>;
  motions: Record<string, Live2dMotionBinding>;
  idle?: Live2dMotionBinding | null;
  eye_blink?: Live2dEyeBlinkBinding | null;
  focus_anchor?: Live2dFocusAnchor | null;
  lip_sync?: Live2dParameterBinding | null;
  /** 抚摸反应：部位键（head/body/legs/earLeft/earRight，可扩展）到绑定。
      不要写 null：Rust 侧是 HashMap 而非 Option，null 反序列化会直接失败。 */
  touch_motions?: Record<string, Live2dTouchBinding>;
  [key: string]: unknown;
}

export interface Live2dSettings {
  version: 1;
  default_variant: string;
  variants: Record<string, Live2dVariant>;
  clothes_variants: Record<string, string>;
  [key: string]: unknown;
}

export interface Live2dImportResult {
  live2d: Live2dSettings;
  models: Array<{
    variant: string;
    model: string;
    expressions: string[];
    motions: Record<string, string[]>;
  }>;
}

/**
 * 一个 variant 的可用资源表，路径相对模型文件所在目录（与 `FileReferences` 同语义）。
 *
 * VTube Studio 式导出的 `model3.json` 里没有 `FileReferences.Expressions/.Motions`
 * （资源散在模型目录下），而引擎只认 `model3.json` 里的声明，名字查不到时
 * `setExpression` 会静默返回 false。所以加载模型时要用这份表把声明补进去。
 * 派生数据，不落 `settings.yml`。
 */
export interface Live2dVariantAssets {
  expressions: Record<string, string>;
  motions: Record<string, string[]>;
}

export function resolveLive2dVariant(
  settings: Live2dSettings,
  clothesName: string,
): Live2dVariant | undefined {
  const normalized = !clothesName || clothesName === "默认" ? "default" : clothesName;
  const mapped = settings.clothes_variants[normalized];
  const variantName = mapped || settings.default_variant;
  return settings.variants[variantName] ?? settings.variants[settings.default_variant];
}

/** 角色形象：Live2D 模型，或静态立绘图片。主对话与桌宠各自独立设置一项。 */
export type AvatarDisplayMode = "live2d" | "image";

/**
 * 该角色在此场景下是否应渲染 Live2D —— 唯一的判定入口，把「配了模型吗」
 * 和「用户选了哪种形象」合成一处，供三个渲染点共用：
 * 主对话 `GameRoleAvatar`、桌宠 `pet/GameRolesStage`、以及真正的模型加载器
 * `Live2DStage.syncRoles`。
 *
 * 参数刻意用结构化类型而不是导入 `GameRole`：`stores/modules/game/state.ts`
 * 反过来要 import 本文件的 `Live2dSettings`，导入 `GameRole` 会形成循环。
 *
 * 只有显式选了 `"image"` 才否定，缺省与未知值一律按 `live2d` —— 即本功能出现前
 * 「有模型就用模型」的行为，老配置无需迁移，手写的错别字也不会让角色突然变样。
 */
export function prefersLive2d(
  role: {
    live2d?: Live2dSettings | null;
    avatarMode?: AvatarDisplayMode | null;
    avatarModeP?: AvatarDisplayMode | null;
  },
  scene: "standard" | "pet" = "standard",
): boolean {
  const mode = scene === "pet" ? role.avatarModeP : role.avatarMode;
  return mode !== "image" && !!role.live2d;
}
