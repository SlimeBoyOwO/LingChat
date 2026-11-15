import type { InjectionKey } from "vue";
import type { PetAvatarTarget } from "./avatarTarget";
import {
  PET_ANIMATION_TEMPLATES,
  type PetAnimationId,
  type PetAnimationTemplate,
} from "./templates";

/** 触发器的某个状态要求播放什么 */
export interface PetTriggerStateSpec {
  animation: PetAnimationId;
  /** hold 保持到状态改变；oneshot 播完即止 */
  mode: "hold" | "oneshot";
}

export interface PetTriggerDefinition<S extends string> {
  id: string;
  /** 数值越大越优先，同优先级保留先注册的 */
  priority: number;
  initial: S;
  /** 状态表；未列出的状态表示该状态不驱动动画 */
  states: Partial<Record<S, PetTriggerStateSpec>>;
}

/** 交给其他代码改状态用的接口 */
export interface PetTriggerHandle<S extends string> {
  readonly state: S;
  setState(state: S): void;
}

export interface PetAnimationController {
  attachTarget(target: PetAvatarTarget): void;
  detachTarget(target: PetAvatarTarget): void;
  registerTrigger<S extends string>(definition: PetTriggerDefinition<S>): PetTriggerHandle<S>;
  /** 门控条件变化后重算，例如设置开关被切走 */
  refresh(): void;
  dispose(): void;
}

/** 门控：决定某个模板此刻能否参与仲裁 */
export type PetAnimationGate = (template: PetAnimationTemplate, target: PetAvatarTarget) => boolean;

export const PET_ANIMATION_KEY: InjectionKey<PetAnimationController> = Symbol("pet-animation");

interface TriggerRecord {
  id: string;
  priority: number;
  states: Record<string, PetTriggerStateSpec | undefined>;
  state: string;
  /** 每次状态变化 +1，一次性动画靠它去重 */
  seq: number;
}

// 桌宠动画控制台：收拢所有触发器的状态机，按优先级决定头像框此刻该做什么。
export function createPetAnimationController(
  options: { gate?: PetAnimationGate } = {},
): PetAnimationController {
  let target: PetAvatarTarget | null = null;
  let held: { triggerId: string; animation: PetAnimationId } | null = null;
  /** 每个触发器已经播到哪个状态序号 */
  const started = new Map<string, number>();
  const triggers = new Map<string, TriggerRecord>();

  const recompute = () => {
    if (!target) return;

    let winner: TriggerRecord | null = null;
    let winnerSpec: PetTriggerStateSpec | null = null;

    for (const record of triggers.values()) {
      const spec = record.states[record.state];
      if (!spec) continue;
      const template = PET_ANIMATION_TEMPLATES[spec.animation];
      if (options.gate && !options.gate(template, target)) continue;
      if (!winner || record.priority > winner.priority) {
        winner = record;
        winnerSpec = spec;
      }
    }

    if (!winner || !winnerSpec) {
      if (held) {
        target.cancel();
        held = null;
      }
      return;
    }

    if (winnerSpec.mode === "hold") {
      if (held?.triggerId === winner.id && held.animation === winnerSpec.animation) return;
      target.hold(winnerSpec.animation);
      held = { triggerId: winner.id, animation: winnerSpec.animation };
      started.set(winner.id, winner.seq);
      return;
    }

    // 一次性动画：同一状态序号只播一次，避免无关重算把回弹重播
    if (started.get(winner.id) === winner.seq) return;
    target.play(winnerSpec.animation);
    held = null;
    started.set(winner.id, winner.seq);
  };

  return {
    attachTarget(next) {
      target?.cancel();
      target = next;
      recompute();
    },

    detachTarget(current) {
      if (target !== current) return;
      target.cancel();
      target = null;
      held = null;
    },

    registerTrigger<S extends string>(definition: PetTriggerDefinition<S>): PetTriggerHandle<S> {
      const record: TriggerRecord = {
        id: definition.id,
        priority: definition.priority,
        states: definition.states as Record<string, PetTriggerStateSpec | undefined>,
        state: definition.initial,
        seq: 0,
      };
      triggers.set(record.id, record);
      // 初始状态视为已播过：注册时不该凭空播一次动画
      started.set(record.id, record.seq);
      recompute();
      return {
        get state() {
          return record.state as S;
        },
        setState(next) {
          if (next === record.state) return;
          record.state = next;
          record.seq += 1;
          recompute();
        },
      };
    },

    refresh: recompute,

    dispose() {
      target?.cancel();
      target = null;
      held = null;
      started.clear();
      triggers.clear();
    },
  };
}
