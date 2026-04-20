import { ref, onMounted, onUnmounted, watch, nextTick, type Ref } from 'vue'
import type {
  FallingParticle,
  FallingParticleConfig,
  UseFallingParticleOptions,
  UseFallingParticleReturn,
  KeyframeConfig,
  ParticleCustomization,
} from '../types/falling'

/**
 * Generate a unique ID for particles
 */
export function generateParticleId(): string {
  return Math.random().toString(36).substr(2, 9)
}

/**
 * Calculate random value within a range
 */
export function randomInRange(min: number, max: number): number {
  return Math.random() * (max - min) + min
}

/**
 * Default particle factory - creates a basic falling particle
 */
export function createDefaultParticle(
  id: string,
  config: FallingParticleConfig,
  customization?: ParticleCustomization,
): FallingParticle {
  const size = randomInRange(config.minSize, config.maxSize)
  const left = Math.random() * window.innerWidth
  const duration = randomInRange(config.minDuration, config.maxDuration)
  const delay = Math.random() * config.maxDelay
  const opacity = randomInRange(config.minOpacity, config.maxOpacity)
  const horizontalMovement = randomInRange(-config.horizontalRange, config.horizontalRange)

  return {
    id,
    size,
    left,
    top: config.initialTopOffset,
    opacity,
    duration,
    delay,
    horizontalMovement,
    ...customization,
  }
}

/**
 * Default keyframe generator - creates CSS keyframes for falling animation
 */
export function createDefaultKeyframes(
  particle: FallingParticle,
  maxHeight: number,
  keyframeConfig: KeyframeConfig,
): string {
  const { rotation, opacity } = keyframeConfig
  const ranges = rotation.rotationRanges
  const kf = opacity.keyframes

  return `
  @keyframes fall-${particle.id} {
    0% {
      transform: translate(0, 0) rotate(${rotation.startRotation}deg);
      opacity: ${particle.opacity * kf[0]!};
    }
    25% {
      transform: translate(${particle.horizontalMovement * 0.25}px, ${maxHeight * 0.25}px) 
                 rotate(${rotation.startRotation + randomInRange(ranges[0]!.min, ranges[0]!.max)}deg);
      opacity: ${particle.opacity * kf[1]!};
    }
    50% {
      transform: translate(${particle.horizontalMovement * 0.5}px, ${maxHeight * 0.5}px) 
                 rotate(${rotation.startRotation + randomInRange(ranges[1]!.min, ranges[1]!.max)}deg);
      opacity: ${particle.opacity * kf[2]!};
    }
    75% {
      transform: translate(${particle.horizontalMovement * 0.75}px, ${maxHeight * 0.75}px) 
                 rotate(${rotation.startRotation + randomInRange(ranges[2]!.min, ranges[2]!.max)}deg);
      opacity: ${particle.opacity * kf[3]!};
    }
    100% {
      transform: translate(${particle.horizontalMovement}px, ${maxHeight}px) 
                 rotate(${rotation.startRotation + randomInRange(ranges[3]!.min, ranges[3]!.max)}deg);
      opacity: ${particle.opacity * kf[4]!};
    }
  }
`
}

export function useFallingParticle<T extends FallingParticle>(
  props: { enabled?: boolean; intensity?: number },
  options: UseFallingParticleOptions<T>,
  containerRef: Ref<HTMLElement | null>,
): UseFallingParticleReturn<T> {
  const { config, baseCount, createParticle, generateKeyframes } = options

  // Normalize props with defaults
  const enabled = props.enabled ?? true
  const intensity = props.intensity ?? 1

  // Reactive state
  const particles = ref<T[]>([]) as Ref<T[]>

  const maxHeight = ref(0)
  const particleCount = ref(Math.floor(baseCount * intensity))

  /**
   * Create animation stylesheet for a particle
   */
  const createParticleAnimation = (particle: T): void => {
    const styleSheet = document.createElement('style')
    document.head.appendChild(styleSheet)

    const keyframes = generateKeyframes(particle, maxHeight.value)
    styleSheet.innerHTML = keyframes
    particle.styleSheet = styleSheet
  }

  /**
   * Create multiple particles
   */
  const createParticles = (count: number): void => {
    for (let i = 0; i < count; i++) {
      const id = generateParticleId()
      const particle = createParticle(id, config)
      particle.id = id
      createParticleAnimation(particle)
      particles.value.push(particle)
    }
  }

  /**
   * Remove all particles and cleanup stylesheets
   */
  const removeAllParticles = (): void => {
    particles.value.forEach((particle) => {
      if (particle.styleSheet && particle.styleSheet.parentNode) {
        particle.styleSheet.parentNode.removeChild(particle.styleSheet)
      }
    })
    particles.value = []
  }

  /**
   * Set max height from container or window
   */
  const setMaxHeight = (): void => {
    if (containerRef.value && containerRef.value.parentElement) {
      maxHeight.value = containerRef.value.parentElement.clientHeight
    } else {
      maxHeight.value = window.innerHeight
    }
  }

  /**
   * Recreate all particles (used for resize events)
   */
  const recreateParticles = (): void => {
    removeAllParticles()
    createParticles(particleCount.value)
  }

  // Watch for intensity changes
  watch(
    () => props.intensity,
    (newIntensity) => {
      particleCount.value = Math.floor(baseCount * (newIntensity ?? 1))
      recreateParticles()
    },
  )

  // Watch for enabled state changes
  watch(
    () => props.enabled,
    (newVal) => {
      if (newVal ?? true) {
        setMaxHeight()
        createParticles(particleCount.value)
      } else {
        removeAllParticles()
      }
    },
  )

  // Lifecycle hooks
  onMounted(() => {
    nextTick(() => {
      setMaxHeight()
      if (enabled) {
        createParticles(particleCount.value)
      }
    })

    window.addEventListener('resize', () => {
      setMaxHeight()
      recreateParticles()
    })
  })

  onUnmounted(() => {
    removeAllParticles()
    window.removeEventListener('resize', setMaxHeight)
  })

  return {
    particles,
    maxHeight,
    createParticles,
    removeAllParticles,
    recreateParticles,
    setMaxHeight,
    particleCount,
  }
}
