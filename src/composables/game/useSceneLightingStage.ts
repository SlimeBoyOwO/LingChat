/**
 * 新光影管线是否接管背景渲染。
 *
 * 模块级单例，因为「接管」是整屏语义：`LightingStage.vue` 挂载并成功启用时置 true，
 * `GameBackground.vue` 据此让出背景层（否则 WebGL 画好的背景会被旧图盖住）。
 *
 * 主窗口、投屏窗口、剧本试玩层可能同时各挂一个 LightingStage，所以用计数而不是布尔值：
 * 只要还有一个实例在跑，背景就归 WebGL 管；全部卸载后交还给旧路径。
 */

import { readonly, ref } from "vue";

const activeInstances = ref(0);

/** 只读状态，供 GameBackground / GameRolesStage 等消费方判断是否该让位。 */
export const lightingStageActive = readonly(activeInstances);

/** 由 `LightingStage.vue` 内部调用；不要在其他组件里手动写。 */
export function acquireLightingStage(): void {
  activeInstances.value += 1;
}

export function releaseLightingStage(): void {
  activeInstances.value = Math.max(0, activeInstances.value - 1);
}
