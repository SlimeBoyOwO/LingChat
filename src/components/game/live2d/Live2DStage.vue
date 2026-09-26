<template>
  <div
    v-bind="$attrs"
    ref="host"
    class="pointer-events-none absolute inset-0 overflow-hidden"
    aria-hidden="true"
  ></div>
  <slot></slot>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { onBeforeUnmount, onMounted, provide, readonly, ref, watch } from "vue";

import { getLive2dFilePath, getLive2dVariantAssets } from "@/api/services/character";
import { EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
import type { GameRole } from "@/stores/modules/game/state";
import {
  prefersLive2d,
  resolveLive2dVariant,
  type Live2dMotionBinding,
  type Live2dVariant,
  type Live2dVariantAssets,
} from "@/types/live2d";
import {
  areEyesOpen,
  gazeFromPointer,
  GAZE_MAGNITUDE_MIN,
  radialReferenceDistance,
  type ScreenBox,
} from "./live2d-interaction";
import { live2dStageContextKey } from "./live2d-stage-context";
import { calculatePetLayout } from "./live2d-layout";
import { trackMotionLifecycle } from "./live2d-motion";
import { loadLive2dRuntime, type Live2dRuntime } from "./live2d-runtime";
import {
  configureRuntimeIdle,
  mergeVariantAssets,
  rewriteModelReferences,
  type Live2dModelSource,
} from "./model-source";
import { decodeVoiceForLipSync, sampleVoiceAmplitude, type DecodedVoice } from "./useLive2dLipSync";

defineOptions({ inheritAttrs: false });

const props = defineProps<{
  roles: GameRole[];
  mode: "standard" | "pet";
  activeSpeakerId: number | null;
  audioElement: HTMLAudioElement | null;
  voiceDataUrl: string;
  /** 投屏全局缩放：乘在角色基础 scale 上，作用于 Live2D 模型本体
      （标准模式下仅投屏窗口传入，主窗口缺省为 1，无影响） */
  castScale?: number;
  /** 投屏全局垂直偏移（像素，正值下移；标准模式下仅投屏窗口传入，主窗口缺省 0）。
      水平偏移由投屏窗口 .cast-role-layer 的 CSS translateX 整层平移，不在此处理。 */
  castOffsetY?: number;
  /** 渲染帧率上限（0 = 不限制）。桌宠窗口很小，30fps 足够且大幅降低挂机 CPU；
      仅桌宠舞台（pet/GameRolesStage）传入，标准模式/预览不传保持原行为 */
  maxFps?: number;
}>();

const emit = defineEmits<{
  activeChange: [roleIds: number[]];
  failedChange: [roleIds: number[]];
}>();

interface CursorPayload {
  x: number;
  y: number;
  /** 当前显示器工作区，已按与 x/y 相同的公式换算到窗口相对逻辑像素。
      旧版 Rust 载荷没有这个字段（undefined），取不到显示器信息时为 null。 */
  screen?: ScreenBox | null;
}

interface RoleModel {
  roleId: number;
  variantName: string;
  model: any;
  variant: Live2dVariant;
  runtimeIdle: Live2dMotionBinding | null;
  /** 当前表情态，存的是**原始**情绪词（分类器/剧本产出的那个）。它是状态变化本身的
      标识，也是查绑定的第一优先键；用 `EMOTION_CONFIG_EMO` 的映射词去重会让
      哭泣与伤心、难为情与羞耻各塌成同一个键，切换时动作不会重放。 */
  emotion: string;
  requestId: number;
  mouthParameterIndex: number;
  mouthValue: number;
  eyeLeftParameterIndex: number;
  eyeRightParameterIndex: number;
  eyeBallXParameterIndex: number;
  eyeBallYParameterIndex: number;
  eyesOpen: boolean;
  focusFrozen: boolean;
  /** 视线幅度：锚点到鼠标的距离 ÷ 该方向上锚点到屏幕边缘的距离，已夹到
      [GAZE_MAGNITUDE_MIN, 1]。1 表示不衰减（维持旧行为）。 */
  gazeMagnitude: number;
  /** 视线原点在模型局部坐标系里的位置，首次用到时由 drawable bounds 与
      focus_anchor 算出后缓存——bounds 随呼吸/动作漂移，每帧重算会让锚点抖动。 */
  focusOrigin: { x: number; y: number } | null;
  reactionSequence: number;
  reactionLifecycleCleanup: (() => void) | null;
}

const host = ref<HTMLDivElement | null>(null);
let runtime: Live2dRuntime | null = null;
let application: any = null;
let disposed = false;
let syncPromise = Promise.resolve();
let requestSequence = 0;
let decodedVoice: DecodedVoice | null = null;
let decodeSequence = 0;
let resizeObserver: ResizeObserver | null = null;
let pointerPosition: { clientX: number; clientY: number } | null = null;
/** 当前显示器工作区（窗口相对逻辑像素），由 Rust 侧的 pet:cursor 广播带过来。
    前端自己读 window.screenX/availLeft 在混合 DPI 多显示器下会混用设备像素与
    CSS 像素；尺寸可靠但位置不可靠，所以位置必须跟指针走同一个来源。 */
let screenBox: ScreenBox | null = null;
let cursorUnlisten: (() => void) | null = null;
const models = new Map<number, RoleModel>();
const failedRoleIds = new Set<number>();
const readyRoleIds = ref<ReadonlySet<number>>(new Set());
const unavailableRoleIds = ref<ReadonlySet<number>>(new Set());

provide(live2dStageContextKey, {
  readyRoleIds: readonly(readyRoleIds),
  unavailableRoleIds: readonly(unavailableRoleIds),
});

function emitFailedRoles() {
  const roleIds = [...failedRoleIds];
  unavailableRoleIds.value = new Set(roleIds);
  emit("failedChange", roleIds);
}

function emitActiveRoles() {
  const roleIds = [...models.keys()];
  readyRoleIds.value = new Set(roleIds);
  emit("activeChange", roleIds);
}

function motionBindingEquals(
  left: Live2dMotionBinding | null | undefined,
  right: Live2dMotionBinding | null | undefined,
) {
  if (!left || !right) return left == null && right == null;
  return (
    left.group === right.group &&
    left.index === right.index &&
    (left.loop ?? true) === (right.loop ?? true)
  );
}

/**
 * 查情绪绑定用的键，按优先级排列。
 *
 * 第一项是分类器与剧本产出的原始情绪词（见 `data/third_party/emotion_model_19emo/label_mapping.json`），
 * 也正是设置界面写进 `settings.yml` 的键，所以它必须先命中。
 *
 * 第二项是 `EMOTION_CONFIG_EMO` 的映射词。那张表是给静态立绘挑气泡图和音效用的
 * （哭泣 → 伤心.webp），Live2D 这里带上它只是让已经写成映射词的配置不回归。少了第一项
 * 会让「哭泣」「难为情」两行变成死键——设置界面绑得上，运行时永远查不到。
 *
 * 表外情绪（如剧本里的「尴尬」）映射词就是「正常」，与映射表出现前的行为一致。
 */
function emotionBindingKeys(emotion: string): string[] {
  const mapped = EMOTION_CONFIG_EMO[emotion] || "正常";
  return emotion === mapped ? [emotion] : [emotion, mapped];
}

/**
 * 按上述顺序取第一个「存在」的绑定。
 *
 * 判空用 `!== undefined` 而不是真值判断：设置界面的「无表情」选项把值写成空串，
 * 那表示用户显式关掉了这个情绪的表情，不能穿透到下一级——这正是 Live2D 文档里
 * 「只在绑定缺失时才回退到 default_expression」的意思。
 */
function pickEmotionBinding<T>(table: Record<string, T>, emotion: string): T | undefined {
  for (const key of emotionBindingKeys(emotion)) {
    const value = table[key];
    if (value !== undefined) return value;
  }
  return undefined;
}

function variantNameFor(role: GameRole): string | null {
  const settings = role.live2d;
  if (!settings || !prefersLive2d(role, props.mode)) return null;
  const clothes = !role.clothesName || role.clothesName === "默认" ? "default" : role.clothesName;
  const mapped = settings.clothes_variants[clothes];
  return mapped || settings.default_variant;
}

async function loadModelSource(
  roleId: number,
  modelFile: string,
  assets: Promise<Live2dVariantAssets | null>,
) {
  const modelPath = await getLive2dFilePath(roleId, modelFile);
  const modelUrl = convertFileSrc(modelPath);
  const response = await fetch(modelUrl);
  if (!response.ok) throw new Error(`Failed to load Live2D settings: HTTP ${response.status}`);
  const source = (await response.json()) as Live2dModelSource;
  // 注入必须夹在解析与改写之间：rewriteModelReferences 会把 FileReferences 里的相对路径
  // 就地转成文件 URL，比它晚注入的路径永远不会被转换，引擎会拿到裸相对路径去取资源
  mergeVariantAssets(source, await assets);
  await rewriteModelReferences(source, modelFile, async (relative) => {
    return convertFileSrc(await getLive2dFilePath(roleId, relative));
  });
  source.url = modelUrl;
  return source;
}

function destroyApplication() {
  resizeObserver?.disconnect();
  resizeObserver = null;
  if (application) {
    application.destroy({ removeView: true, releaseGlobalResources: false }, true);
    application = null;
  }
  pointerPosition = null;
  runtime = null;
}

async function ensureApplication() {
  if (application || !host.value || disposed) return;
  runtime = await loadLive2dRuntime();
  const app = new runtime.pixi.Application();
  await app.init({
    resizeTo: host.value,
    preference: "webgl",
    backgroundAlpha: 0,
    antialias: true,
    autoDensity: true,
    resolution: Math.min(window.devicePixelRatio, props.mode === "pet" ? 1.5 : 2),
  });
  if (disposed || !host.value) {
    app.destroy({ removeView: true, releaseGlobalResources: false }, true);
    return;
  }
  app.canvas.className = "absolute inset-0 w-full h-full";
  host.value.appendChild(app.canvas);
  app.ticker.speed = 1.35;
  // 帧率上限：0/undefined 视为不限帧（保持标准模式/预览原行为）
  const fpsCap = props.maxFps ?? 0;
  if (fpsCap > 0) app.ticker.maxFPS = fpsCap;
  app.ticker.add(updateLipSync);
  resizeObserver = new ResizeObserver(() => {
    for (const entry of models.values()) {
      const role = props.roles.find((item) => item.roleId === entry.roleId);
      if (role) applyLayout(entry, role);
    }
  });
  resizeObserver.observe(host.value);
  application = app;
}

function findParameterIndex(entry: RoleModel, parameter: string): number {
  const core = entry.model.internalModel.coreModel;
  for (let index = 0; index < core.getParameterCount(); index += 1) {
    if (core.getParameterId(index).isEqual(parameter)) return index;
  }
  return -1;
}

/** 工作区矩形缺失时（移动端、浏览器 dev）的径向参考距离。
    window.screen 的尺寸在多 DPI 下可靠、位置不可靠，所以只取尺寸；
    连尺寸都拿不到时返回 0，gazeFromPointer 据此退回「不衰减」。 */
function fallbackReferenceDistance() {
  const screen = window.screen;
  return radialReferenceDistance(screen?.availWidth ?? 0, screen?.availHeight ?? 0);
}

/** 视线原点（模型局部坐标）= drawable bounds 上的 focus_anchor 位置。
    没配 focus_anchor 时取 bounds 中心，与设置界面的 placeholder 一致。
    getLocalBounds() 读的是当前（带呼吸/动作）的 drawable 顶点，会随动画漂移，
    所以只算一次缓存；局部 bounds 不受 model.scale/position/anchor 影响，
    桌宠改缩放不会让它失效。 */
function resolveFocusOrigin(entry: RoleModel) {
  if (entry.focusOrigin) return entry.focusOrigin;
  const bounds = entry.model.getLocalBounds();
  const anchor = entry.variant.focus_anchor ?? { x: 0.5, y: 0.5 };
  entry.focusOrigin = {
    x: (bounds.minX ?? bounds.x ?? 0) + bounds.width * anchor.x,
    y: (bounds.minY ?? bounds.y ?? 0) + bounds.height * anchor.y,
  };
  return entry.focusOrigin;
}

function updateModelFocus(entry: RoleModel) {
  if (entry.focusFrozen) return;
  const focusController = entry.model.internalModel.focusController;
  // 标准聊天模式：始终直视前方（不跟随鼠标）；仅桌宠模式用指针驱动视线
  if (props.mode !== "pet") {
    focusController.focus(0, 0);
    return;
  }
  // 眨眼/隐藏期间冻结视线目标：不重置回中。引擎的眨眼控制器会在
  // beforeModelUpdate 之前把眼部参数写成闭眼值，此时若走回中分支，
  // 弹簧插值会把瞳孔/头短暂拽向正中，表现为眨眼瞬间“瞬视中间”。
  // 下面每条早退分支都保持 gazeMagnitude 不变：焦点弹簧自己会衰减到 0，
  // 瞳孔补偿量随之归零，路径连续；清零反而会漏掉补偿、多出一个小跳变。
  if (!entry.eyesOpen || !entry.model.visible) return;
  if (!pointerPosition || !host.value || !application) {
    focusController.focus(0, 0);
    return;
  }
  // 全程在视口坐标里算距离：指针与工作区矩形都在这个坐标系。
  // 反算用 application.screen 而非 rect 做分母——PIXI 的 ResizePlugin 读
  // clientWidth，application.screen 不受 CSS transform 影响，而 rect 会
  // （桌宠入场有 scale(0.8→1) 动画，期间两者差 0.8 倍）。
  const rect = host.value.getBoundingClientRect();
  const stage = application.screen;
  if (rect.width <= 0 || rect.height <= 0 || stage.width <= 0 || stage.height <= 0) {
    focusController.focus(0, 0);
    return;
  }
  const origin = entry.model.toGlobal(resolveFocusOrigin(entry));
  const gaze = gazeFromPointer(
    { x: pointerPosition.clientX, y: pointerPosition.clientY },
    {
      x: rect.left + origin.x * (rect.width / stage.width),
      y: rect.top + origin.y * (rect.height / stage.height),
    },
    screenBox,
    screenBox ? 0 : fallbackReferenceDistance(),
  );
  // 方向按单位向量交给引擎驱动瞳孔；幅度只用来衰减头部旋转，
  // 被缩掉的瞳孔偏转由 beforeModelUpdate 补回。
  const magnitude = Math.max(gaze.magnitude, GAZE_MAGNITUDE_MIN);
  entry.gazeMagnitude = magnitude;
  focusController.focus(gaze.x * magnitude, gaze.y * magnitude);
}

function handlePointerMove(event: PointerEvent) {
  pointerPosition = { clientX: event.clientX, clientY: event.clientY };
}

function destroyModel(model: any) {
  // Textures loaded through Pixi Assets are shared across stages and owned by the global cache.
  model.destroy({ children: true, texture: false, baseTexture: false });
}

function applyLayout(entry: RoleModel, role: GameRole) {
  if (!application || !host.value) return;
  const model = entry.model;
  const bounds = model.getLocalBounds();
  const width = bounds.width || model.internalModel.width || model.width || 1;
  const height = bounds.height || model.internalModel.height || model.height || 1;
  if (props.mode === "pet") {
    const layout = calculatePetLayout(
      application.screen,
      { width, height },
      role.scaleP || 1,
      role.offsetXP || 0,
      role.offsetYP || 0,
    );
    model.anchor.set(layout.anchorX, layout.anchorY);
    model.scale.set(layout.scale);
    model.position.set(layout.x, layout.y);
  } else {
    const index = props.roles.findIndex((item) => item.roleId === role.roleId);
    const count = props.roles.length;
    const xPercent = index < 0 ? 0.5 : (index + 1) / (count + 1);
    // 投屏 scale 折进原公式里的 roleScale（与 model.scale / position.y 同步相乘，
    // 保持贴底定位）；主窗口缺省 castScale=1，行为与原先完全一致
    const roleScale = (role.scale || 1) * (props.castScale ?? 1);
    const baseScale = application.screen.height / height;
    model.anchor.set(0.5, 1);
    model.scale.set(baseScale * roleScale);
    // 投屏垂直偏移（castOffsetY，正值下移）折进角色脚底位置，但只夹紧「投屏自己下移
    // 的那段」：角色自身配置的 role.offsetY 不参与夹紧，保持原语义。脚底默认在
    // H*roleScale + role.offsetY；投屏下移最多补到窗口底沿（脚底触底即止），人物下方
    // 不会被窗口 overflow:hidden 截断；上移（负值）自由。水平偏移由投屏窗口的
    // .cast-role-layer CSS translateX 整层平移（见 CastWindow.vue）。
    const defaultFootY = application.screen.height * roleScale + (role.offsetY || 0);
    const downLimit = application.screen.height - defaultFootY;
    const castOffsetY = props.castOffsetY ?? 0;
    const effectiveOffsetY =
      castOffsetY > 0 ? Math.min(castOffsetY, Math.max(0, downLimit)) : castOffsetY;
    model.position.set(
      application.screen.width * xPercent + (role.offsetX || 0),
      defaultFootY + effectiveOffsetY,
    );
  }
  model.visible = role.show;
}

function startIdle(entry: RoleModel) {
  if (!entry.runtimeIdle || !runtime) return;
  const idle = entry.runtimeIdle;
  void entry.model.motion(idle.group, idle.index, runtime.engine.MotionPriority.IDLE, {
    loop: idle.loop ?? true,
    resetExpression: false,
  });
}

function freezeModelFocus(entry: RoleModel) {
  const focusController = entry.model.internalModel.focusController;
  focusController.focus(focusController.x, focusController.y, true);
  entry.focusFrozen = true;
}

function finishReaction(entry: RoleModel, sequence: number) {
  if (sequence !== entry.reactionSequence) return;
  entry.reactionLifecycleCleanup?.();
  entry.reactionLifecycleCleanup = null;
  entry.focusFrozen = false;
  updateModelFocus(entry);
}

function applyEmotion(entry: RoleModel, emotion: string) {
  if (entry.emotion === emotion || !runtime) return;
  entry.emotion = emotion;
  const expression =
    pickEmotionBinding(entry.variant.expressions, emotion) ?? entry.variant.default_expression;
  if (expression) {
    void entry.model
      .expression(expression)
      .catch((error: unknown) =>
        console.warn(`[Live2D] expression failed for role ${entry.roleId}`, error),
      );
  }
  const motion = pickEmotionBinding(entry.variant.motions, emotion);
  if (motion) {
    const sequence = ++entry.reactionSequence;
    freezeModelFocus(entry);
    const motionManager = entry.model.internalModel.motionManager;
    entry.reactionLifecycleCleanup?.();
    entry.reactionLifecycleCleanup = trackMotionLifecycle(
      motionManager,
      motion.group,
      motion.index,
      runtime.engine.MotionPriority.FORCE,
      () => finishReaction(entry, sequence),
    );
    void entry.model
      .motion(motion.group, motion.index, runtime.engine.MotionPriority.FORCE, {
        loop: motion.loop ?? false,
        resetExpression: false,
      })
      .then((started: boolean) => {
        if (!started) finishReaction(entry, sequence);
      })
      .catch((error: unknown) => {
        finishReaction(entry, sequence);
        console.warn(`[Live2D] motion failed for role ${entry.roleId}`, error);
      });
  }
}

function destroyEntry(entry: RoleModel) {
  entry.reactionSequence += 1;
  entry.reactionLifecycleCleanup?.();
  entry.reactionLifecycleCleanup = null;
  application?.stage.removeChild(entry.model);
  destroyModel(entry.model);
  models.delete(entry.roleId);
  emitActiveRoles();
}

async function loadRole(
  role: GameRole,
  variantName: string,
  variant: Live2dVariant,
  requestId: number,
) {
  await ensureApplication();
  if (!application || !runtime || disposed) return;
  let pendingModel: any = null;
  const previous = models.get(role.roleId);
  let previousDetached = false;
  try {
    // 与模型文件并行取回。资源表拿不到不该拖垮模型加载：它只是补声明，缺了顶多
    // 某个表情选了不生效，而抛出去会让角色退化成静态立绘，明显更糟
    const assets = getLive2dVariantAssets(role.roleId, variantName).catch((error: unknown) => {
      console.warn(`[Live2D] failed to load variant assets for role ${role.roleId}`, error);
      return null;
    });
    const source = await loadModelSource(role.roleId, variant.model, assets);
    // 必须在注入之后：扫描出来的待机组（小写 idle）只有注入完才解析得到，
    // 解析不到会抛错，被下面的 catch 兜成静态立绘
    const runtimeIdle = configureRuntimeIdle(source, variant.idle);
    const model = await runtime.engine.Live2DModel.from(source, {
      ticker: application.ticker,
      anchorMode: "drawable",
      autoFocus: false,
      autoHitTest: false,
      eyeBlink: true,
      idleMotionGroup: runtimeIdle?.group ?? variant.idle?.group ?? "Idle",
      motionPreload: runtime.engine.MotionPreloadStrategy.IDLE,
      useHighPrecisionMask: "auto",
      textureOptions: { lod: "single-auto" },
    });
    pendingModel = model;
    const currentRole = props.roles.find((item) => item.roleId === role.roleId);
    if (
      disposed ||
      requestId !== requestSequenceFor(role.roleId) ||
      !currentRole ||
      variantNameFor(currentRole) !== variantName
    ) {
      destroyModel(model);
      pendingModel = null;
      return;
    }
    const entry: RoleModel = {
      roleId: role.roleId,
      variantName,
      model,
      variant,
      runtimeIdle,
      emotion: "",
      requestId,
      mouthParameterIndex: -1,
      mouthValue: 0,
      eyeLeftParameterIndex: -1,
      eyeRightParameterIndex: -1,
      eyeBallXParameterIndex: -1,
      eyeBallYParameterIndex: -1,
      eyesOpen: true,
      focusFrozen: false,
      gazeMagnitude: 1,
      focusOrigin: null,
      reactionSequence: 0,
      reactionLifecycleCleanup: null,
    };
    if (variant.lip_sync?.parameter) {
      entry.mouthParameterIndex = findParameterIndex(entry, variant.lip_sync.parameter);
    }
    if (variant.eye_blink) {
      entry.eyeLeftParameterIndex = findParameterIndex(entry, variant.eye_blink.left);
      entry.eyeRightParameterIndex = findParameterIndex(entry, variant.eye_blink.right);
    }
    // 瞳孔参数名是 Cubism 标准 id，不像 eye_blink 那样需要按模型配置；
    // 缺失时索引为 -1，该模型就只衰减头部、瞳孔也跟着衰减（降级而非报错）
    entry.eyeBallXParameterIndex = findParameterIndex(entry, "ParamEyeBallX");
    entry.eyeBallYParameterIndex = findParameterIndex(entry, "ParamEyeBallY");
    model.internalModel.on("beforeModelUpdate", () => {
      const coreModel = model.internalModel.coreModel as {
        addParameterValueByIndex(index: number, value: number, weight?: number): void;
        getParameterValueByIndex(index: number): number;
      };
      if (entry.mouthParameterIndex >= 0) {
        coreModel.addParameterValueByIndex(entry.mouthParameterIndex, entry.mouthValue, 1);
      }
      const eyeValues: number[] = [];
      if (entry.eyeLeftParameterIndex >= 0) {
        eyeValues.push(coreModel.getParameterValueByIndex(entry.eyeLeftParameterIndex));
      }
      if (entry.eyeRightParameterIndex >= 0) {
        eyeValues.push(coreModel.getParameterValueByIndex(entry.eyeRightParameterIndex));
      }
      entry.eyesOpen = areEyesOpen(eyeValues);
      // 瞳孔补偿。引擎已按衰减后的焦点写了一次眼球参数，这里把被缩掉的那份补回，
      // 使瞳孔仍然是满幅追踪（头部不受影响，只衰减那一份）。
      // 幅度必须在 updateModelFocus 覆写之前读：focusController.update(dt) 在帧首
      // 执行，追赶的是上一帧 handler 里设的 target，所以本帧的 fc 对应的是旧幅度。
      // fc 是「径向 + 限速」的弹簧，从原点出发时恒为 s·magnitude·u，故 fc/magnitude
      // 恰是未衰减时瞳孔应有的值，且 |fc/magnitude| ≤ 1，这次写入不会被参数 clamp 削掉。
      const magnitude = entry.gazeMagnitude;
      if (props.mode === "pet" && magnitude < 1) {
        const focusController = model.internalModel.focusController;
        const gain = 1 / magnitude - 1;
        if (entry.eyeBallXParameterIndex >= 0) {
          coreModel.addParameterValueByIndex(
            entry.eyeBallXParameterIndex,
            focusController.x * gain,
            1,
          );
        }
        if (entry.eyeBallYParameterIndex >= 0) {
          coreModel.addParameterValueByIndex(
            entry.eyeBallYParameterIndex,
            focusController.y * gain,
            1,
          );
        }
      }
      updateModelFocus(entry);
    });
    if (previous) {
      application.stage.removeChild(previous.model);
      previousDetached = true;
    }
    application.stage.addChild(model);
    applyLayout(entry, role);
    // Verify the new model in isolation before replacing the active variant.
    application.render();
    if (previous) destroyEntry(previous);
    models.set(role.roleId, entry);
    pendingModel = null;
    startIdle(entry);
    applyEmotion(entry, role.emotion);
    failedRoleIds.delete(role.roleId);
    emitFailedRoles();
    emitActiveRoles();
  } catch (error) {
    if (pendingModel) {
      application?.stage.removeChild(pendingModel);
      destroyModel(pendingModel);
    }
    if (requestId === requestSequenceFor(role.roleId)) {
      const current = models.get(role.roleId);
      if (current) {
        if (previousDetached && !application.stage.children.includes(current.model)) {
          application.stage.addChild(current.model);
          applyLayout(current, role);
        }
        failedRoleIds.delete(role.roleId);
      } else {
        failedRoleIds.add(role.roleId);
      }
      emitFailedRoles();
    }
    console.warn(
      `[Live2D] model load failed for role ${role.roleId}; keeping static avatar`,
      error,
    );
  }
}

const roleRequests = new Map<number, number>();
function nextRequest(roleId: number) {
  const id = ++requestSequence;
  roleRequests.set(roleId, id);
  return id;
}
function requestSequenceFor(roleId: number) {
  return roleRequests.get(roleId);
}

async function syncRoles() {
  // 形象由角色设定决定（主对话/桌宠各自一项），切成静态立绘的角色在此被排除，
  // 模型根本不加载；全部排除时下面会 destroyApplication()。
  const liveRoles = props.roles.filter((role) => prefersLive2d(role, props.mode));
  const liveIds = new Set(liveRoles.map((role) => role.roleId));
  let failedChanged = false;
  for (const roleId of [...failedRoleIds]) {
    if (!liveIds.has(roleId)) {
      failedRoleIds.delete(roleId);
      failedChanged = true;
    }
  }
  if (failedChanged) emitFailedRoles();
  for (const entry of [...models.values()]) {
    if (!liveIds.has(entry.roleId)) {
      nextRequest(entry.roleId);
      failedRoleIds.delete(entry.roleId);
      emitFailedRoles();
      destroyEntry(entry);
    }
  }
  if (!liveRoles.length) {
    destroyApplication();
    return;
  }
  await ensureApplication();
  for (const [index, role] of liveRoles.entries()) {
    const settings = role.live2d;
    if (!settings) continue;
    const variantName = variantNameFor(role);
    const variant = variantName ? resolveLive2dVariant(settings, role.clothesName) : undefined;
    if (!variantName || !variant) continue;
    const entry = models.get(role.roleId);
    if (
      !entry ||
      entry.variantName !== variantName ||
      entry.variant.model !== variant.model ||
      !motionBindingEquals(entry.variant.idle, variant.idle)
    ) {
      await loadRole(role, variantName, variant, nextRequest(role.roleId));
      continue;
    }
    if (entry.variant !== variant) {
      entry.variant = variant;
      entry.emotion = "";
      // 变体换了但模型没换（例如只改了 focus_anchor）：缓存的视线原点已经失效，
      // 不清掉的话新锚点要等模型下次重新加载才生效
      entry.focusOrigin = null;
      startIdle(entry);
    }
    applyLayout(entry, role);
    applyEmotion(entry, role.emotion);
    application.stage.setChildIndex(
      entry.model,
      Math.min(index, application.stage.children.length - 1),
    );
  }
}

function queueSync() {
  syncPromise = syncPromise
    .then(syncRoles)
    .catch((error) => console.warn("[Live2D] stage sync failed", error));
}

function updateLipSync() {
  const audio = props.audioElement;
  for (const entry of models.values()) {
    const isSpeaker =
      entry.roleId === props.activeSpeakerId && audio && !audio.paused && !audio.ended;
    const target = isSpeaker
      ? sampleVoiceAmplitude(decodedVoice, audio.currentTime) * (entry.variant.lip_sync?.gain ?? 1)
      : 0;
    entry.mouthValue += (Math.min(1, target) - entry.mouthValue) * 0.38;
  }
}

watch(
  () =>
    props.roles.map(
      (role) =>
        [
          role.roleId,
          role.emotion,
          role.clothesName,
          role.show,
          role.scale,
          role.offsetX,
          role.offsetY,
          role.scaleP,
          role.offsetXP,
          role.offsetYP,
          role.live2d,
          role.avatarMode,
          role.avatarModeP,
        ] as const,
    ),
  queueSync,
  { deep: true },
);

// 投屏全局缩放 / 垂直偏移变化时重新布局（滑块拖动即时生效，复用 queueSync 幂等重排；
// 水平偏移由投屏窗口 CSS translateX 处理，不在此触发）
watch(() => [props.castScale, props.castOffsetY] as const, queueSync);

// 帧率上限设置热更新：设置窗口改完即时生效，无需重进桌宠模式
watch(
  () => props.maxFps ?? 0,
  (fps) => {
    if (!application) return;
    // 0 = 不限帧（PIXI Ticker 语义：maxFPS=0 即关闭上限）
    application.ticker.maxFPS = fps > 0 ? fps : 0;
  },
);

watch(
  () => [props.voiceDataUrl, props.roles.some((role) => prefersLive2d(role, props.mode))] as const,
  async ([url, hasLive2dRole]) => {
    const id = ++decodeSequence;
    decodedVoice = null;
    if (!hasLive2dRole) return;
    const decoded = await decodeVoiceForLipSync(url);
    if (id === decodeSequence) decodedVoice = decoded;
  },
);

onMounted(() => {
  // 桌宠模式：窗口非全屏，DOM pointermove 在鼠标移出窗口后停发，视线会冻结在
  // 最后一次窗口内位置。除窗口内 DOM 监听外，还需订阅 Rust 侧全局鼠标轮询
  // （每 50ms 上报窗口内逻辑坐标，即 webview 视口坐标，与 clientX/clientY 同源）。
  if (props.mode === "pet") {
    window.addEventListener("pointermove", handlePointerMove, { passive: true });
    void listen<CursorPayload>("pet:cursor", (event) => {
      pointerPosition = { clientX: event.payload.x, clientY: event.payload.y };
      // 旧版 Rust 载荷没有 screen 字段：保持上一次的值，别把参考系清掉
      if (event.payload.screen !== undefined) screenBox = event.payload.screen ?? null;
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        cursorUnlisten = unlisten;
      })
      .catch(() => {
        // 非 Tauri 环境或事件系统不可用时静默降级（DOM 监听仍覆盖窗口内移动）
      });
  }
  queueSync();
});
onBeforeUnmount(() => {
  disposed = true;
  if (props.mode === "pet") {
    window.removeEventListener("pointermove", handlePointerMove);
    cursorUnlisten?.();
    cursorUnlisten = null;
  }
  decodeSequence += 1;
  for (const entry of [...models.values()]) destroyEntry(entry);
  destroyApplication();
});
</script>
