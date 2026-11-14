/**
 * 桌宠动画模板：纯数据，不读状态、不碰 DOM。
 * 新增动画就是在这里加一条，再由触发器状态表引用。
 */

export interface PetAnimationTemplate {
  keyframes: Keyframe[];
  duration: number;
  easing: string;
  /** 门控标签：同标签的动画共用一条策略，由触发层统一判断 */
  tag?: "jelly";
}

export const PET_ANIMATION_TEMPLATES = {
  /** 按下：整体等比缩小，保持到松手 */
  press: {
    keyframes: [{ transform: "scale(1, 1)" }, { transform: "scale(0.93, 0.93)" }],
    duration: 130,
    easing: "ease-out",
    tag: "jelly",
  },
  /** 松开：从捏小状态回弹，带一次挤压过冲 */
  release: {
    keyframes: [
      { transform: "scale(0.93, 0.93)" },
      { transform: "scale(1.03, 0.97)", offset: 0.3 },
      { transform: "scale(0.985, 1.015)", offset: 0.55 },
      { transform: "scale(1.01, 0.99)", offset: 0.78 },
      { transform: "scale(1, 1)" },
    ],
    duration: 480,
    easing: "ease-out",
    tag: "jelly",
  },
} satisfies Record<string, PetAnimationTemplate>;

export type PetAnimationId = keyof typeof PET_ANIMATION_TEMPLATES;
