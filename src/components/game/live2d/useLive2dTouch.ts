/** 抚摸交互的运行时：指针监听、晃动归属、闭眼、视线、音效与粒子撒点，舞台只管命中判定与画面反馈。 */

import { useSettingsStore } from "@/stores/modules/settings";
import {
  gazeFromPointer,
  screenFallbackReferenceDistance,
  type ScreenBox,
} from "./live2d-interaction";
import { createStrokeTracker } from "./live2d-touch";

/** 一次抚摸锁定的目标：哪个角色的哪个部位 */
export interface TouchTarget {
  roleId: number;
  part: string;
}

export interface TouchHost {
  /** 指针落点命中的角色与部位，未命中可摸目标时为 null */
  hitTest(clientX: number, clientY: number): TouchTarget | null;
  /** 角色视线原点在视口里的位置，取不到舞台几何时为 null */
  gazeAnchor(roleId: number): { x: number; y: number } | null;
  /** 按下并落在部位上，晃动随之归属该角色 */
  onStrokeStart(target: TouchTarget): void;
  /** 晃动已回正，可以起这次抚摸的动作了 */
  onReactionDue(target: TouchTarget): void;
  /** 抚摸表情到期，换回情绪的表情 */
  onExpressionDue(roleId: number): void;
  /** 该角色是否还有反应动作在播 */
  reactionInFlight(roleId: number): boolean;
  spawnParticle(clientX: number, clientY: number): void;
}

export interface TouchSession {
  /** 指针位置，标准模式下由本模块维护，桌宠模式由舞台的全局鼠标监听写入 */
  readonly pointer: { x: number; y: number } | null;
  /** 当前显示器工作区，只由桌宠模式的鼠标广播写入 */
  readonly screenBox: ScreenBox | null;
  /** 该角色是否由抚摸驱动焦点，是则给出当前晃动方向 */
  swayFor(roleId: number): { x: number; y: number } | null;
  /** 该角色的闭眼程度，0 为睁着 */
  eyeCloseFor(roleId: number): number;
  /** 该角色瞳孔应看的方向，权重已含淡入淡出；不需要接管时为 null */
  gazeFor(roleId: number): { x: number; y: number; weight: number } | null;
  setPointer(clientX: number, clientY: number): void;
  setScreenBox(box: ScreenBox | null): void;
  /** 进入或退出触摸模式 */
  setEnabled(enabled: boolean): void;
  /** 每帧调用一次 */
  update(): void;
}

/** 粒子的间距，CSS 像素。按走过的距离撒而不是按时间撒，手停下就不再冒。 */
const PARTICLE_TRAIL_DISTANCE = 46;

/** 抚摸音效的播放间隔，毫秒 */
const PET_SOUND_INTERVAL_MS = 300;

/** 五声音阶的来回走向。跨度压在一个五度内，避免连着摸时越摸越尖。 */
const PET_SOUND_RATES = [1, 1.1225, 1.2599, 1.4983, 1.2599, 1.1225];

const PET_SOUND_URL = `${import.meta.env.BASE_URL}audio/pet.mp3`;

/** 一次抚摸闭眼保持多久，毫秒。松手提前结束，不把这 2 秒补完。 */
const EYE_CLOSE_HOLD_MS = 2000;

/** 睁眼与闭眼的缓动时长，毫秒 */
const EYE_LID_MS = 160;

/** 进触摸模式时视线转向玩家的淡入时长，毫秒 */
const GAZE_FADE_MS = 180;

/** 晃动衰减到这个幅度以下就让动作开演，此时剩下的偏角已经看不出来 */
const REACTION_SWAY_THRESHOLD = 0.06;

/** 上一条的兜底上限，只是给动作排期，不是用计时器去猜动作播完没有 */
const REACTION_DELAY_LIMIT_MS = 900;

/** 启发式黑名单：监听挂在 window 上，会连带吃到输入框、按钮、对话框与设置界面的拖拽 */
const INTERACTIVE_SELECTOR =
  "input, textarea, select, button, a, [contenteditable='true'], .game-dialog, .settings-panel";

export function createTouchSession(host: TouchHost): TouchSession {
  const stroke = createStrokeTracker();
  let pointer: { x: number; y: number } | null = null;
  let screenBox: ScreenBox | null = null;
  /** 由抚摸驱动焦点的角色，晃回正才交还，中途交还头会跳一下 */
  let ownerRoleId: number | null = null;
  /** 表情被抚摸占着的角色，它比晃动活得久：要等动作也播完才收回去 */
  let expressionRoleId: number | null = null;
  /** 待播的抚摸动作，等晃动回正再起，免得顶着最后那点偏角播完 */
  let pending: (TouchTarget & { since: number }) | null = null;
  let enabled = false;
  let soundStep = 0;
  let soundElapsed = 0;
  let lid = 0;
  let lidUntil = 0;
  let gazeWeight = 0;
  let frameAt = 0;
  let trailX = 0;
  let trailY = 0;
  let trailDistance = 0;

  function setPointer(clientX: number, clientY: number) {
    pointer = { x: clientX, y: clientY };
  }

  function setScreenBox(box: ScreenBox | null) {
    screenBox = box;
  }

  // 音量挂在气泡音效上，它就是这个项目里短音效的总线
  function playSound() {
    const voice = new Audio(PET_SOUND_URL);
    voice.volume = useSettingsStore().bubbleVolume / 100;
    voice.playbackRate = PET_SOUND_RATES[soundStep % PET_SOUND_RATES.length]!;
    soundStep += 1;
    void voice.play().catch(() => {}); // 没有用户手势时浏览器会拒绝播放，跳过这一声
  }

  function handleDown(event: PointerEvent) {
    if (event.button !== 0) return;
    setPointer(event.clientX, event.clientY); // 标准模式没有别的地方维护指针位置
    if (document.elementFromPoint(event.clientX, event.clientY)?.closest(INTERACTIVE_SELECTOR)) {
      return;
    }
    const now = performance.now();
    const target = host.hitTest(event.clientX, event.clientY);
    if (!stroke.begin(target?.part ?? null, event.clientX, event.clientY, now) || !target) return;
    ownerRoleId = target.roleId;
    expressionRoleId = target.roleId;
    pending = null; // 新的一次抚摸作废上一次还没播出去的动作
    host.onStrokeStart(target);
    trailX = event.clientX;
    trailY = event.clientY;
    trailDistance = 0;
    soundStep = 0; // 每次摸都从根音起，听感一致
    soundElapsed = 0;
    playSound();
    lidUntil = now + EYE_CLOSE_HOLD_MS;
  }

  function handleMove(event: PointerEvent) {
    setPointer(event.clientX, event.clientY); // 先记位置再早退，松手后视线还要淡出一小会儿
    if (!stroke.active) return;
    const target = host.hitTest(event.clientX, event.clientY);
    trailDistance += Math.hypot(event.clientX - trailX, event.clientY - trailY);
    trailX = event.clientX;
    trailY = event.clientY;
    if (target && trailDistance >= PARTICLE_TRAIL_DISTANCE) {
      trailDistance = 0;
      host.spawnParticle(event.clientX, event.clientY);
    }
    stroke.move(target?.part ?? null, event.clientX, event.clientY, performance.now());
  }

  /** 抬手。晃动不在这里清零，交给 update 自然回正。 */
  function handleRelease() {
    if (!stroke.active) return;
    const { part, stroked } = stroke.end();
    if (stroked && part && ownerRoleId !== null) {
      pending = { roleId: ownerRoleId, part, since: performance.now() };
    }
  }

  function releaseExpression() {
    if (expressionRoleId === null) return;
    const roleId = expressionRoleId;
    expressionRoleId = null;
    host.onExpressionDue(roleId);
  }

  function attach() {
    window.addEventListener("pointerdown", handleDown, { passive: true });
    window.addEventListener("pointermove", handleMove, { passive: true });
    window.addEventListener("pointerup", handleRelease, { passive: true });
    // 拖动中失焦或指针被浏览器收走时只有 pointercancel/blur，缺了这两条 active 会永久为 true
    window.addEventListener("pointercancel", handleRelease, { passive: true });
    window.addEventListener("blur", handleRelease);
  }

  function detach() {
    window.removeEventListener("pointerdown", handleDown);
    window.removeEventListener("pointermove", handleMove);
    window.removeEventListener("pointerup", handleRelease);
    window.removeEventListener("pointercancel", handleRelease);
    window.removeEventListener("blur", handleRelease);
    stroke.cancel();
    pending = null;
    releaseExpression(); // 离开触摸模式时表情立刻收回，否则会一直挂在脸上
  }

  function setEnabled(next: boolean) {
    if (enabled === next) return;
    enabled = next;
    if (next) attach();
    else detach();
  }

  function swayFor(roleId: number) {
    return ownerRoleId === roleId ? stroke.sway : null;
  }

  function eyeCloseFor(roleId: number) {
    return ownerRoleId === roleId ? lid : 0;
  }

  function gazeFor(roleId: number) {
    if (gazeWeight <= 0 || !pointer) return null;
    const anchor = host.gazeAnchor(roleId);
    if (!anchor) return null;
    const gaze = gazeFromPointer(
      pointer,
      anchor,
      screenBox,
      screenBox ? 0 : screenFallbackReferenceDistance(),
    );
    return { x: gaze.x, y: gaze.y, weight: gazeWeight };
  }

  function update() {
    const now = performance.now();
    // dt 每帧都要推进，否则空闲一段时间后的第一帧会拿到一个陈旧的 dt
    const dt = Math.min(64, Math.max(0, now - frameAt));
    frameAt = now;

    const wantGaze = enabled ? 1 : 0;
    const idle =
      !stroke.active &&
      !pending &&
      ownerRoleId === null &&
      expressionRoleId === null &&
      gazeWeight === wantGaze &&
      lid === 0;
    if (idle) return;

    gazeWeight += (wantGaze - gazeWeight) * (1 - Math.exp(-dt / GAZE_FADE_MS));
    if (Math.abs(wantGaze - gazeWeight) < 0.002) gazeWeight = wantGaze; // 收敛到目标就定住

    stroke.update(now);

    if (stroke.active && stroke.part !== null) {
      soundElapsed += dt;
      if (soundElapsed >= PET_SOUND_INTERVAL_MS) {
        soundElapsed -= PET_SOUND_INTERVAL_MS;
        playSound();
      }
    } else {
      soundElapsed = 0;
    }

    // 必须线性步进而不是指数逼近：指数到不了目标，lid 就永远不等于 0，上面的空闲判定再也无法成立
    const wantLid = stroke.active && stroke.part !== null && now < lidUntil ? 1 : 0;
    if (lid !== wantLid) {
      const step = dt / EYE_LID_MS;
      lid = wantLid > lid ? Math.min(wantLid, lid + step) : Math.max(wantLid, lid - step);
    }

    if (!stroke.active && stroke.settled) ownerRoleId = null; // 回正才交还焦点通道，否则头会跳一下
    if (pending) {
      // 动作等晃动几乎回正就起；超时只是排期兜底，不是用它去猜动作播完没有
      const ready =
        stroke.magnitude < REACTION_SWAY_THRESHOLD ||
        now - pending.since >= REACTION_DELAY_LIMIT_MS;
      if (ready) {
        const target = pending;
        pending = null;
        host.onReactionDue(target);
      }
    }
    // 表情比晃动活得久，要等动作也播完。必须带 !stroke.active：按住不动时晃动同样会回正
    if (expressionRoleId !== null && !stroke.active && stroke.settled) {
      if (!host.reactionInFlight(expressionRoleId)) releaseExpression();
    }
  }

  return {
    get pointer() {
      return pointer;
    },
    get screenBox() {
      return screenBox;
    },
    setPointer,
    setScreenBox,
    swayFor,
    eyeCloseFor,
    gazeFor,
    setEnabled,
    update,
  };
}
