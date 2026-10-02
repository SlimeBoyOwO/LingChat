import type { Ref } from "vue";
import { useParticleField } from "./useParticleField";
import { FallingField } from "./useFallingField";
import type { FallingEffect, FallingProps } from "../types/falling";

/**
 * 把下落粒子的渲染场挂到画布上。
 *
 * 雪与樱花共用这一份实现，差异只在传入的 effect。
 * 尺寸观察与生命周期交给 useParticleField。
 */
export function useFallingParticle(
  canvasRef: Ref<HTMLCanvasElement | null>,
  props: FallingProps,
  effect: FallingEffect,
) {
  useParticleField(
    canvasRef,
    props,
    (canvas, intensity) => new FallingField(canvas, effect, intensity),
  );
}
