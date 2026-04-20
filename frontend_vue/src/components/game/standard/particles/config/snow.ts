import type { FallingParticleConfig, KeyframeConfig } from '../types/falling'

const snowConfig: FallingParticleConfig = {
  minSize: 12,
  maxSize: 28,
  minDuration: 20,
  maxDuration: 35,
  maxDelay: 10,
  minOpacity: 0.3,
  maxOpacity: 1.0,
  horizontalRange: 50,
  initialTopOffset: -30,
}

const snowSettings = {
  baseCount: 50,
  chars: ['❄', '❅', '❆', '•', '·'] as const,
}

const snowKeyframeConfig: KeyframeConfig = {
  rotation: {
    startRotation: 0,
    rotationRanges: [
      { min: 0, max: 90 },
      { min: 0, max: 180 },
      { min: 0, max: 270 },
      { min: 0, max: 360 },
    ],
  },
  opacity: {
    keyframes: [1, 0.95, 0.9, 0.8, 0.4],
  },
}

export const snow = {
  config: snowConfig,
  settings: snowSettings,
  keyframes: snowKeyframeConfig,
}
