import type { CatalogAsset } from '@/api/services/tts-catalog'

export interface VoiceInstalledSnapshot {
  voice_id: string
  has_style_vectors: boolean
}

export interface LocalTtsStatusLite {
  deberta_installed: boolean
}

export type CatalogState =
  | 'missing'
  | 'downloading'
  | 'installed'
  | 'error'

export interface CatalogRowInputs {
  asset: CatalogAsset
  progressPercent?: number | null
  errorMessage?: string | null
  status?: LocalTtsStatusLite | null
  voices?: VoiceInstalledSnapshot[]
}

const findVoice = (
  voices: VoiceInstalledSnapshot[],
  id: string,
): VoiceInstalledSnapshot | undefined =>
  voices.find((v) => v.voice_id === id)

export function catalogRowState(input: CatalogRowInputs): CatalogState {
  const { asset, progressPercent, errorMessage, status, voices } = input
  if (errorMessage) return 'error'
  if (typeof progressPercent === 'number' && progressPercent < 100) {
    return 'downloading'
  }

  const voiceList = voices ?? []
  if (asset.kind === 'bert') {
    if (asset.id === 'deberta' || asset.id === 'deberta-tokenizer') {
      return status?.deberta_installed ? 'installed' : 'missing'
    }
    return 'missing'
  }
  if (asset.kind === 'voice') {
    const voice = findVoice(voiceList, asset.id)
    if (!voice) return 'missing'
    return 'installed'
  }
  if (asset.kind === 'style_vectors') {
    const voiceId = asset.voice_id
    if (!voiceId) return 'missing'
    const voice = findVoice(voiceList, voiceId)
    if (!voice) return 'missing'
    return voice.has_style_vectors ? 'installed' : 'missing'
  }
  return 'missing'
}
