import { invoke } from '@tauri-apps/api/core'

export interface IndexTtsVoicePreset {
  id: number
  file_name: string
  size: number
}

export const listIndexTtsVoices = (): Promise<IndexTtsVoicePreset[]> => {
  return invoke('indextts_voice_list')
}

export const uploadIndexTtsVoice = (
  fileName: string,
  fileData: Uint8Array,
): Promise<IndexTtsVoicePreset[]> => {
  return invoke('indextts_voice_upload', { fileName, fileData })
}

export const deleteIndexTtsVoice = (fileName: string): Promise<IndexTtsVoicePreset[]> => {
  return invoke('indextts_voice_delete', { fileName })
}
