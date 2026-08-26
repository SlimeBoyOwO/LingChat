import type { Live2dMotionBinding } from '@/types/live2d'

export interface Live2dModelReferences {
  Moc?: string
  Textures?: string[]
  Physics?: string
  Pose?: string
  UserData?: string
  DisplayInfo?: string
  Expressions?: Array<{ File?: string; [key: string]: unknown }>
  Motions?: Record<string, Array<{ File?: string; Sound?: string; [key: string]: unknown }>>
  [key: string]: unknown
}

export interface Live2dModelSource {
  FileReferences?: Live2dModelReferences
  url?: string
  [key: string]: unknown
}

export const RUNTIME_IDLE_GROUP = '__LingChatConfiguredIdle'

export function configureRuntimeIdle(
  source: Live2dModelSource,
  idle: Live2dMotionBinding | null | undefined,
): Live2dMotionBinding | null {
  if (!idle) return null
  const motions = source.FileReferences?.Motions
  const definition = motions?.[idle.group]?.[idle.index]
  if (!motions || !definition) {
    throw new Error(`Configured Live2D idle motion does not exist: ${idle.group}[${idle.index}]`)
  }
  motions[RUNTIME_IDLE_GROUP] = [{ ...definition }]
  return { group: RUNTIME_IDLE_GROUP, index: 0, loop: idle.loop ?? true }
}

const URL_SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/

export function resolveModelReference(modelFile: string, reference: string): string {
  if (
    URL_SCHEME.test(reference) ||
    reference.startsWith('/') ||
    /^[a-zA-Z]:[\\/]/.test(reference)
  ) {
    throw new Error(`Live2D resource reference must be relative: ${reference}`)
  }

  const segments = modelFile.split('\\').join('/').split('/')
  segments.pop()
  for (const segment of reference.split('\\').join('/').split('/')) {
    if (!segment || segment === '.') continue
    if (segment === '..') {
      if (!segments.length) {
        throw new Error(`Live2D resource escapes the role directory: ${reference}`)
      }
      segments.pop()
    } else {
      segments.push(segment)
    }
  }
  return segments.join('/')
}

export async function rewriteModelReferences(
  source: Live2dModelSource,
  modelFile: string,
  resolveFileUrl: (roleRelativePath: string) => Promise<string>,
): Promise<Live2dModelSource> {
  const references = source.FileReferences
  if (!references) throw new Error('Live2D model3 is missing FileReferences')

  const rewrite = (reference: string) => resolveFileUrl(resolveModelReference(modelFile, reference))
  const rewrites: Promise<void>[] = []

  for (const key of ['Moc', 'Physics', 'Pose', 'UserData', 'DisplayInfo'] as const) {
    const reference = references[key]
    if (typeof reference === 'string') {
      rewrites.push(
        rewrite(reference).then((url) => {
          references[key] = url
        }),
      )
    }
  }

  references.Textures?.forEach((reference, index) => {
    rewrites.push(
      rewrite(reference).then((url) => {
        references.Textures![index] = url
      }),
    )
  })

  references.Expressions?.forEach((expression) => {
    if (typeof expression.File === 'string') {
      rewrites.push(
        rewrite(expression.File).then((url) => {
          expression.File = url
        }),
      )
    }
  })

  for (const motions of Object.values(references.Motions ?? {})) {
    for (const motion of motions) {
      if (typeof motion.File === 'string') {
        rewrites.push(
          rewrite(motion.File).then((url) => {
            motion.File = url
          }),
        )
      }
      if (typeof motion.Sound === 'string' && motion.Sound.length > 0) {
        rewrites.push(
          rewrite(motion.Sound).then((url) => {
            motion.Sound = url
          }),
        )
      }
    }
  }

  await Promise.all(rewrites)
  return source
}
