import type { Ref } from 'vue'

/**
 * Base interface for falling particle properties
 */
export interface FallingParticle {
  /** Unique identifier for the particle (auto-generated) */
  id?: string
  /** Size of the particle in pixels */
  size: number
  /** Horizontal position (left) in pixels */
  left: number
  /** Vertical position (top) in pixels */
  top: number
  /** Opacity value (0-1) */
  opacity: number
  /** Animation duration in seconds */
  duration: number
  /** Animation delay in seconds */
  delay: number
  /** Horizontal movement range in pixels */
  horizontalMovement: number
  /** Dynamically created stylesheet element (managed by hook) */
  styleSheet?: HTMLStyleElement
  /** Content character (like snowflake) */
  content?: string
  /** Hue value for color */
  hue?: number
}

/**
 * Configuration options for falling particle animation
 */
export interface FallingParticleConfig {
  /** Minimum size of particles in pixels */
  minSize: number
  /** Maximum size of particles in pixels */
  maxSize: number
  /** Minimum animation duration in seconds */
  minDuration: number
  /** Maximum animation duration in seconds */
  maxDuration: number
  /** Maximum animation delay in seconds */
  maxDelay: number
  /** Minimum opacity value */
  minOpacity: number
  /** Maximum opacity value */
  maxOpacity: number
  /** Horizontal movement range (+/- pixels) */
  horizontalRange: number
  /** Initial top position offset */
  initialTopOffset: number
}

/**
 * Options for the useFallingParticle hook
 */
export interface UseFallingParticleOptions<T extends FallingParticle> {
  /** Configuration for particle generation */
  config: FallingParticleConfig
  /** Base count of particles (multiplied by intensity) */
  baseCount: number
  /** Factory function to create a custom particle */
  createParticle: (id: string, config: FallingParticleConfig) => T
  /** Factory function to generate keyframe animation CSS */
  generateKeyframes: (particle: T, maxHeight: number) => string
}

/**
 * Keyframe rotation configuration
 */
export interface RotationConfig {
  startRotation: number
  rotationRanges: Array<{ min: number; max: number }>
}

/**
 * Keyframe opacity configuration
 */
export interface OpacityConfig {
  keyframes: number[]
}

/**
 * Keyframe configuration for falling animations
 */
export interface KeyframeConfig {
  rotation: RotationConfig
  opacity: OpacityConfig
}

/**
 * Particle customization options
 */
export interface ParticleCustomization {
  /** Content character (for snowflakes) */
  content?: string
  /** Hue value for color (for petals) */
  hue?: number
}

/**
 * Return type for useFallingParticle hook
 */
export interface UseFallingParticleReturn<T extends FallingParticle> {
  /** Reactive array of particles */
  particles: Ref<T[]>
  /** Current maximum height */
  maxHeight: Ref<number>
  /** Create particles */
  createParticles: (count: number) => void
  /** Remove all particles */
  removeAllParticles: () => void
  /** Recreate all particles */
  recreateParticles: () => void
  /** Update max height from container */
  setMaxHeight: () => void
  /** Current particle count based on intensity */
  particleCount: Ref<number>
}
