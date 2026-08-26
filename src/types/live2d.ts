export interface Live2dMotionBinding {
  group: string
  index: number
  loop?: boolean
  [key: string]: unknown
}

export interface Live2dParameterBinding {
  parameter: string
  gain?: number
  [key: string]: unknown
}

export interface Live2dEyeBlinkBinding {
  left: string
  right: string
  [key: string]: unknown
}

export interface Live2dFocusAnchor {
  x: number
  y: number
  [key: string]: unknown
}

export interface Live2dVariant {
  model: string
  default_expression?: string | null
  expressions: Record<string, string>
  motions: Record<string, Live2dMotionBinding>
  idle?: Live2dMotionBinding | null
  eye_blink?: Live2dEyeBlinkBinding | null
  focus_anchor?: Live2dFocusAnchor | null
  lip_sync?: Live2dParameterBinding | null
  [key: string]: unknown
}

export interface Live2dSettings {
  version: 1
  default_variant: string
  variants: Record<string, Live2dVariant>
  clothes_variants: Record<string, string>
  [key: string]: unknown
}

export interface Live2dImportResult {
  live2d: Live2dSettings
  models: Array<{
    variant: string
    model: string
    expressions: string[]
    motions: Record<string, string[]>
  }>
}

export function resolveLive2dVariant(
  settings: Live2dSettings,
  clothesName: string,
): Live2dVariant | undefined {
  const normalized = !clothesName || clothesName === '默认' ? 'default' : clothesName
  const mapped = settings.clothes_variants[normalized]
  const variantName = mapped || settings.default_variant
  return settings.variants[variantName] ?? settings.variants[settings.default_variant]
}
