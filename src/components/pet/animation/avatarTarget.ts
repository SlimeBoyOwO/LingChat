import type { Ref } from "vue";
import { PET_ANIMATION_TEMPLATES, type PetAnimationId } from "./templates";

// 头像框动画目标
export interface PetAvatarTarget {
  /** 播一次，不保留末帧 */
  play(id: PetAnimationId): void;
  /** 保持末帧，直到 cancel 或被下一次动画替换 */
  hold(id: PetAnimationId): void;
  cancel(): void;
  /** 供触发层门控：模型是否真的在渲染 */
  isModelRendering(): boolean;
}

const reduceMotion =
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export function createPetAvatarTarget(
  el: Ref<HTMLElement | null>,
  isModelRendering: () => boolean,
): PetAvatarTarget {
  let animation: Animation | null = null;

  const run = (id: PetAnimationId, fill: FillMode) => {
    const node = el.value;
    if (!node || reduceMotion) return;
    animation?.cancel();
    const template = PET_ANIMATION_TEMPLATES[id];
    animation = node.animate(template.keyframes, {
      duration: template.duration,
      easing: template.easing,
      fill,
    });
  };

  return {
    play: (id) => run(id, "none"),
    hold: (id) => run(id, "forwards"),
    cancel: () => {
      animation?.cancel();
      animation = null;
    },
    isModelRendering,
  };
}
