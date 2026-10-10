<template>
  <div
    v-bind="$attrs"
    ref="host"
    class="pointer-events-none absolute inset-0 overflow-hidden"
    aria-hidden="true"
  >
    <!-- 抚摸粒子放在画布容器内部：它要盖住模型却压在气泡之下，而 PIXI 的画布是
         运行时才追加到这个 div 末尾的，只能靠 z-1 压过它那个 auto -->
    <TouchParticles ref="touchParticles" />
  </div>
  <slot></slot>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { onBeforeUnmount, onMounted, provide, readonly, ref, watch } from "vue";

import { useCharacterEditorApi } from "@/composables/useCharacterEditor";
const { getLive2dFilePath, getLive2dVariantAssets } = useCharacterEditorApi();
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
  screenFallbackReferenceDistance,
  type ScreenBox,
} from "./live2d-interaction";
import { live2dStageContextKey } from "./live2d-stage-context";
import { calculatePetLayout } from "./live2d-layout";
import { emotionExpression, pickEmotionBinding } from "./live2d-emotion";
import { trackMotionLifecycle } from "./live2d-motion";
import { loadLive2dRuntime, type Live2dRuntime } from "./live2d-runtime";
import TouchParticles from "./TouchParticles.vue";
import {
  hitTouchPart,
  resolveTouchRegions,
  type TouchBounds,
  type TouchRegion,
} from "./live2d-touch";
import {
  configureRuntimeIdle,
  mergeVariantAssets,
  rewriteModelReferences,
  type Live2dModelSource,
} from "./model-source";
import { createTouchSession } from "./useLive2dTouch";
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
  /** 抚摸交互开关。缺省 false 是刻意的：设置界面的预览挂同一个组件，不传就自动免疫。 */
  touchEnabled?: boolean;
  /** 仅配置预览开放实例内的调试方法，不影响其他舞台。 */
  editorPreview?: boolean;
}>();

const emit = defineEmits<{
  activeChange: [roleIds: number[]];
  failedChange: [roleIds: number[]];
  previewGeometry: [value: { roleId: number; x: number; y: number }];
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
  /** 当前表情态，存的是原始情绪词。用映射词去重会让哭泣与伤心塌成同一个键，动作不再重放。 */
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
  /** 抚摸命中区域与推导它所用的 bounds，与 focusOrigin 同一次算出，无绑定或无锚点时为 null */
  touchRegions: Record<string, TouchRegion> | null;
  touchBounds: TouchBounds | null;
  geometryConfig: string;
  previewGeometry: { x: number; y: number } | null;
  reactionSequence: number;
  reactionLifecycleCleanup: (() => void) | null;
}

const host = ref<HTMLDivElement | null>(null);
const touchParticles = ref<InstanceType<typeof TouchParticles> | null>(null);
let runtime: Live2dRuntime | null = null;
let application: any = null;
let disposed = false;
let syncPromise = Promise.resolve();
let requestSequence = 0;
let decodedVoice: DecodedVoice | null = null;
let decodeSequence = 0;
let resizeObserver: ResizeObserver | null = null;
let cursorUnlisten: (() => void) | null = null;
const models = new Map<number, RoleModel>();
const failedRoleIds = new Set<number>();
const readyRoleIds = ref<ReadonlySet<number>>(new Set());
const unavailableRoleIds = ref<ReadonlySet<number>>(new Set());

/** 抚摸交互的全部跨帧状态都在这里，舞台只提供命中判定与画面反馈。 */
const touch = createTouchSession({
  hitTest: partAtPoint,
  gazeAnchor: (roleId) => {
    const entry = models.get(roleId);
    return entry ? focusOriginViewport(entry) : null;
  },
  onStrokeStart: ({ roleId, part }) => {
    const entry = models.get(roleId);
    if (entry) applyTouchExpression(entry, part);
  },
  onReactionDue: ({ roleId, part }) => {
    const entry = models.get(roleId);
    if (entry) playTouchReaction(entry, part);
  },
  onExpressionDue: (roleId) => {
    const entry = models.get(roleId);
    if (entry) applyExpression(entry, emotionExpression(entry.variant, entry.emotion));
  },
  reactionInFlight: (roleId) => !!models.get(roleId)?.reactionLifecycleCleanup,
  spawnParticle: (clientX, clientY) => touchParticles.value?.spawn(clientX, clientY),
});

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
  app.ticker.add(() => touch.update());
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

/** 舞台几何：宿主矩形与 PIXI 逻辑尺寸。分母一律取 screen，rect 会被 CSS transform 缩放。 */
function stageGeometry() {
  if (!host.value || !application) return null;
  const rect = host.value.getBoundingClientRect();
  const stage = application.screen;
  if (rect.width <= 0 || rect.height <= 0 || stage.width <= 0 || stage.height <= 0) return null;
  return { rect, stage };
}

/** 解析并缓存模型局部几何。只算一次：bounds 随呼吸与动作漂移，每帧重算会让锚点抖动。 */
function resolveLocalGeometry(entry: RoleModel) {
  if (entry.focusOrigin && entry.touchBounds) return entry.focusOrigin;
  const bounds = entry.model.getLocalBounds();
  const minX = bounds.minX ?? bounds.x ?? 0;
  const minY = bounds.minY ?? bounds.y ?? 0;
  const anchor = entry.variant.focus_anchor ?? null;
  entry.focusOrigin = {
    x: minX + bounds.width * (anchor?.x ?? 0.5),
    y: minY + bounds.height * (anchor?.y ?? 0.5),
  };
  entry.touchBounds = { minX, minY, width: bounds.width, height: bounds.height };
  // 区域用 variant 上的原始锚点，不回落：回落后头区会落在躯干正中，宁可一个都不生成
  entry.touchRegions = resolveTouchRegions(anchor, entry.variant.touch_motions);
  return entry.focusOrigin;
}

/** 视线原点换算到视口坐标 */
function focusOriginViewport(entry: RoleModel): { x: number; y: number } | null {
  const geometry = stageGeometry();
  if (!geometry) return null;
  const { rect, stage } = geometry;
  const origin = entry.model.toGlobal(resolveLocalGeometry(entry));
  return {
    x: rect.left + origin.x * (rect.width / stage.width),
    y: rect.top + origin.y * (rect.height / stage.height),
  };
}

function updateModelFocus(entry: RoleModel) {
  if (entry.focusFrozen) return;
  const focusController = entry.model.internalModel.focusController;
  // 标准模式默认直视前方，抚摸期间把焦点通道临时交给手；桌宠模式才用指针驱动视线
  if (props.mode !== "pet") {
    const sway = touch.swayFor(entry.roleId);
    focusController.focus(sway?.x ?? 0, sway?.y ?? 0);
    return;
  }
  const pointer = touch.pointer;
  // 眨眼或隐藏期间冻结视线目标而不是回中，否则弹簧插值会把瞳孔与头短暂拽向正中
  if (!entry.eyesOpen || !entry.model.visible) return;
  const anchor = focusOriginViewport(entry);
  if (!pointer || !anchor) {
    focusController.focus(0, 0);
    return;
  }
  const gaze = gazeFromPointer(
    pointer,
    anchor,
    touch.screenBox,
    touch.screenBox ? 0 : screenFallbackReferenceDistance(),
  );
  // 方向按单位向量交给引擎；幅度只衰减头部旋转，被缩掉的瞳孔偏转由 beforeModelUpdate 补回
  const magnitude = Math.max(gaze.magnitude, GAZE_MAGNITUDE_MIN);
  entry.gazeMagnitude = magnitude;
  focusController.focus(gaze.x * magnitude, gaze.y * magnitude);
}

function handlePointerMove(event: PointerEvent) {
  touch.setPointer(event.clientX, event.clientY);
}

/** 视口坐标换算到模型局部坐标，与 focusOriginViewport 互为逆运算。 */
function localPointFor(entry: RoleModel, clientX: number, clientY: number) {
  const geometry = stageGeometry();
  if (!geometry) return null;
  const { rect, stage } = geometry;
  return entry.model.toLocal({
    x: (clientX - rect.left) * (stage.width / rect.width),
    y: (clientY - rect.top) * (stage.height / rect.height),
  });
}

/** 指针落在哪个角色的哪个部位。多角色站位重叠时取舞台层级最高的那个。 */
function partAtPoint(clientX: number, clientY: number) {
  if (!application) return null;
  const candidates = [...models.values()]
    .filter((entry) => entry.model.visible && entry.variant.touch_motions)
    // 用 children.indexOf 而不是 getChildIndex：后者在模型不在舞台上时会抛错
    .sort(
      (a, b) =>
        application.stage.children.indexOf(b.model) - application.stage.children.indexOf(a.model),
    );
  for (const entry of candidates) {
    resolveLocalGeometry(entry); // 标准模式的视线路径走不到这里，必须自己触发，否则区域恒为空
    const local = localPointFor(entry, clientX, clientY);
    if (!local || !entry.touchBounds || !entry.touchRegions) continue;
    const part = hitTouchPart(local, entry.touchBounds, entry.touchRegions);
    if (part) return { roleId: entry.roleId, part };
  }
  return null;
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
  if (props.editorPreview && application.screen.width && application.screen.height) {
    const point = model.toGlobal(resolveLocalGeometry(entry));
    const geometry = {
      x: point.x / application.screen.width,
      y: point.y / application.screen.height,
    };
    // 布局 watcher 会被父组件的预览标记更新触发；相同坐标不重复发送，避免渲染反馈循环。
    if (entry.previewGeometry?.x !== geometry.x || entry.previewGeometry?.y !== geometry.y) {
      entry.previewGeometry = geometry;
      emit("previewGeometry", { roleId: role.roleId, ...geometry });
    }
  }
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

/** 启动一次 FORCE 优先级的动作反应。步骤顺序见 docs/live2d/development.md 的 Reaction Completion，不要改成超时或直接写参数。 */
function startReaction(entry: RoleModel, binding: Live2dMotionBinding) {
  if (!runtime) return Promise.resolve(false);
  const sequence = ++entry.reactionSequence;
  freezeModelFocus(entry);
  entry.reactionLifecycleCleanup?.();
  entry.reactionLifecycleCleanup = trackMotionLifecycle(
    entry.model.internalModel.motionManager,
    binding.group,
    binding.index,
    runtime.engine.MotionPriority.FORCE,
    () => finishReaction(entry, sequence),
  );
  return entry.model
    .motion(binding.group, binding.index, runtime.engine.MotionPriority.FORCE, {
      loop: binding.loop ?? false,
      resetExpression: false,
    })
    .then((started: boolean) => {
      if (!started) finishReaction(entry, sequence);
      return started;
    })
    .catch((error: unknown) => {
      finishReaction(entry, sequence);
      console.warn(`[Live2D] motion failed for role ${entry.roleId}`, error);
      return false;
    });
}

/** 播放某个部位绑定的抚摸动作。优先级必须 FORCE，否则会被在播的待机动作直接拒掉。 */
function playTouchReaction(entry: RoleModel, part: string): boolean {
  const binding = entry.variant.touch_motions?.[part];
  const { group, index } = binding ?? {};
  if (!binding || group === undefined || index === undefined || !runtime) return false;
  if (entry.reactionLifecycleCleanup) {
    // 已有反应在跑就静默丢弃，保住剧本的情绪节拍，顺带当冷却用
    console.debug(`[Live2D] touch reaction dropped, reaction in flight (role ${entry.roleId})`);
    return false;
  }
  startReaction(entry, { group, index, loop: binding.loop ?? false });
  return true;
}

function applyExpression(entry: RoleModel, expression: string | null | undefined) {
  if (!expression) return;
  void entry.model
    .expression(expression)
    .catch((error: unknown) =>
      console.warn(`[Live2D] expression failed for role ${entry.roleId}`, error),
    );
}

/** 换上抚摸表情。按下时就换比等到动作开演更跟手，收回去的时机由抚摸会话决定。 */
function applyTouchExpression(entry: RoleModel, part: string) {
  const expression = entry.variant.touch_motions?.[part]?.expression;
  if (expression) applyExpression(entry, expression);
}

function applyEmotion(entry: RoleModel, emotion: string) {
  if (entry.emotion === emotion || !runtime) return;
  entry.emotion = emotion;
  applyExpression(entry, emotionExpression(entry.variant, emotion));
  const motion = pickEmotionBinding(entry.variant.motions, emotion);
  // 情绪反应无条件抢占：entry.emotion 已经写进去了，这次丢掉就再也不会重播
  if (motion) startReaction(entry, motion);
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

/** 引擎更新模型前的最后一道写入。顺序不能改：瞳孔补偿与视线覆写都依赖 updateModelFocus 尚未生效时的焦点值。 */
function applyModelParameters(entry: RoleModel, model: any) {
  // 全部用「加上差值」而不是赋值：不必把 coreModel 拓宽到 set 方法，中间值也不会越界
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

  // 抚摸闭眼：眨眼控制器本帧已经写过开合参数，这里是覆盖它
  const closed = touch.eyeCloseFor(entry.roleId);
  if (closed > 0) {
    const openness = 1 - closed;
    for (const index of [entry.eyeLeftParameterIndex, entry.eyeRightParameterIndex]) {
      if (index < 0) continue;
      coreModel.addParameterValueByIndex(
        index,
        openness - coreModel.getParameterValueByIndex(index),
        1,
      );
    }
  }

  const focusController = model.internalModel.focusController;
  // 瞳孔补偿：把引擎按衰减后焦点写的那份补回，fc/magnitude 恰是未衰减时瞳孔应有的值
  const magnitude = entry.gazeMagnitude;
  if (props.mode === "pet" && magnitude < 1) {
    const gain = 1 / magnitude - 1;
    if (entry.eyeBallXParameterIndex >= 0) {
      coreModel.addParameterValueByIndex(entry.eyeBallXParameterIndex, focusController.x * gain, 1);
    }
    if (entry.eyeBallYParameterIndex >= 0) {
      coreModel.addParameterValueByIndex(entry.eyeBallYParameterIndex, focusController.y * gain, 1);
    }
  }

  // 触摸模式下瞳孔看向指针：把引擎这一帧写的那份减掉换成指针方向，精确抵消，头摆头的
  const gaze = touch.gazeFor(entry.roleId);
  if (gaze) {
    const deltaX = gaze.x * gaze.weight - focusController.x;
    const deltaY = gaze.y * gaze.weight - focusController.y;
    if (entry.eyeBallXParameterIndex >= 0) {
      coreModel.addParameterValueByIndex(entry.eyeBallXParameterIndex, deltaX, 1);
    }
    if (entry.eyeBallYParameterIndex >= 0) {
      coreModel.addParameterValueByIndex(entry.eyeBallYParameterIndex, deltaY, 1);
    }
  }

  updateModelFocus(entry);
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
      touchRegions: null,
      touchBounds: null,
      geometryConfig: JSON.stringify([variant.focus_anchor, variant.touch_motions]),
      previewGeometry: null,
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
    model.internalModel.on("beforeModelUpdate", () => applyModelParameters(entry, model));
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
    if (!props.editorPreview) applyEmotion(entry, role.emotion);
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
      // 变体换了但模型没换：缓存的几何已失效，不清掉新锚点要等模型重载才生效
      entry.focusOrigin = null;
      entry.touchRegions = null;
      entry.touchBounds = null;
      startIdle(entry);
    }
    const geometryConfig = JSON.stringify([variant.focus_anchor, variant.touch_motions]);
    if (geometryConfig !== entry.geometryConfig) {
      entry.geometryConfig = geometryConfig;
      entry.focusOrigin = null;
      entry.touchBounds = null;
      entry.touchRegions = null;
    }
    applyLayout(entry, role);
    if (!props.editorPreview) applyEmotion(entry, role.emotion);
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

// 预览操作只作用于此组件持有的模型，不发送全局事件、不修改游戏角色状态。
function previewEntry(roleId: number) {
  return props.editorPreview ? models.get(roleId) : undefined;
}
async function previewExpression(roleId: number, expression: string) {
  const entry = previewEntry(roleId);
  if (!entry) return false;
  return Boolean(await entry.model.expression(expression));
}
async function previewEmotion(roleId: number, emotion: string) {
  const entry = previewEntry(roleId);
  if (!entry) return false;
  entry.emotion = emotion;
  const expression = emotionExpression(entry.variant, emotion);
  let applied = false;
  if (expression) applied = Boolean(await entry.model.expression(expression));
  else entry.model.internalModel.motionManager.expressionManager?.resetExpression();
  const motion = pickEmotionBinding(entry.variant.motions, emotion);
  return motion
    ? (await startReaction(entry, { ...motion, loop: false })) || applied
    : applied || !expression;
}
async function previewMotion(roleId: number, binding: Live2dMotionBinding) {
  const entry = previewEntry(roleId);
  if (!entry || !Number.isInteger(binding.index) || binding.index < 0) return false;
  return await startReaction(entry, { ...binding, loop: false });
}
async function previewTouch(roleId: number, part: string) {
  const entry = previewEntry(roleId);
  const binding = entry?.variant.touch_motions?.[part];
  if (!entry || !binding) return false;
  let applied = false;
  if (binding.expression) applied = Boolean(await entry.model.expression(binding.expression));
  if (binding.group !== undefined && binding.index !== undefined)
    return await startReaction(entry, { group: binding.group, index: binding.index, loop: false });
  return applied;
}
function pickFocusAnchor(roleId: number, clientX: number, clientY: number) {
  const entry = previewEntry(roleId);
  const geometry = stageGeometry();
  if (!entry || !geometry) return null;
  resolveLocalGeometry(entry);
  const bounds = entry.touchBounds;
  if (!bounds || !bounds.width || !bounds.height) return null;
  const local = entry.model.toLocal({
    x: ((clientX - geometry.rect.left) * geometry.stage.width) / geometry.rect.width,
    y: ((clientY - geometry.rect.top) * geometry.stage.height) / geometry.rect.height,
  });
  const x = (local.x - bounds.minX) / bounds.width;
  const y = (local.y - bounds.minY) / bounds.height;
  return x >= 0 && x <= 1 && y >= 0 && y <= 1
    ? { x: Math.round(x * 1000) / 1000, y: Math.round(y * 1000) / 1000 }
    : null;
}
defineExpose({ previewExpression, previewEmotion, previewMotion, previewTouch, pickFocusAnchor });

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

// 进出触摸模式即时生效，不必重进 /chat。桌宠模式本次不接抚摸，两个条件必须同时成立。
function isStrokeEnabled() {
  return props.mode === "standard" && props.touchEnabled === true;
}

watch(isStrokeEnabled, (enabled) => touch.setEnabled(enabled));

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
      touch.setPointer(event.payload.x, event.payload.y);
      // 旧版 Rust 载荷没有 screen 字段：保持上一次的值，别把参考系清掉
      if (event.payload.screen !== undefined) touch.setScreenBox(event.payload.screen ?? null);
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
  touch.setEnabled(isStrokeEnabled());
  queueSync();
});
onBeforeUnmount(() => {
  disposed = true;
  if (props.mode === "pet") {
    window.removeEventListener("pointermove", handlePointerMove);
    cursorUnlisten?.();
    cursorUnlisten = null;
  }
  touch.setEnabled(false);
  decodeSequence += 1;
  for (const entry of [...models.values()]) destroyEntry(entry);
  destroyApplication();
});
</script>
