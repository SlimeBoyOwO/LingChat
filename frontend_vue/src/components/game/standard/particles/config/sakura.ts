import type { FallingParticleConfig, KeyframeConfig } from '../types/falling'

const sakuraConfig: FallingParticleConfig = {
  minSize: 10,
  maxSize: 20,
  minDuration: 15,
  maxDuration: 25,
  maxDelay: 10,
  minOpacity: 0.4,
  maxOpacity: 0.9,
  horizontalRange: 50,
  initialTopOffset: -30,
}

const sakuraSettings = {
  baseCount: 25,
  hueMin: 320,
  hueMax: 330,
}

const sakuraKeyframeConfig: KeyframeConfig = {
  rotation: {
    startRotation: 0,
    rotationRanges: [
      { min: 90, max: 180 },
      { min: 180, max: 270 },
      { min: 270, max: 360 },
      { min: 360, max: 540 },
    ],
  },
  opacity: {
    keyframes: [1, 0.9, 0.7, 0.5, 0],
  },
}

export const sakura = {
  config: sakuraConfig,
  settings: sakuraSettings,
  keyframes: sakuraKeyframeConfig,
}
