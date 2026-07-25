export type AssetKind = 'bert' | 'voice' | 'style_vectors'

export interface CatalogAsset {
  id: string
  kind: AssetKind
  display_name: string
  language: string
  size_bytes: number
  sha256: string
  download_url: string
  source: string
  voice_id?: string
}

export const TTS_LOCAL_CATALOG: CatalogAsset[] = [
  {
    id: 'deberta',
    kind: 'bert',
    display_name: 'DeBERTa-v3-base (Japanese BERT)',
    language: 'ja',
    size_bytes: 278_000_000,
    sha256: '',
    download_url:
      'https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/deberta.onnx',
    source: 'lingchat-research-studio/DeBERTa.onnx',
  },
  {
    id: 'deberta-tokenizer',
    kind: 'bert',
    display_name: 'DeBERTa-v3-base Tokenizer',
    language: 'ja',
    size_bytes: 2_100_000,
    sha256: '',
    download_url:
      'https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/tokenizer.json',
    source: 'lingchat-research-studio/DeBERTa.onnx',
  },
  {
    id: 'ling-v2',
    kind: 'voice',
    display_name: 'Ling-v2 (Japanese)',
    language: 'ja',
    size_bytes: 249_000_000,
    sha256: '',
    download_url:
      'https://www.modelscope.cn/models/lingchat-research-studio/sbv2api-model-Ling-v2-onnx/resolve/master/sbv2api-model-Ling-v2-onnx.onnx',
    source: 'lingchat-research-studio/sbv2api-model-Ling-v2-onnx',
  },
  {
    id: 'ling-v2-style',
    kind: 'style_vectors',
    display_name: 'Ling-v2 Style Vectors',
    language: 'ja',
    size_bytes: 7_400,
    sha256: '',
    download_url:
      'https://www.modelscope.cn/models/lingchat-research-studio/sbv2api-model-Ling-v2-onnx/resolve/master/style_vectors.json',
    source: 'lingchat-research-studio/sbv2api-model-Ling-v2-onnx',
    voice_id: 'ling-v2',
  },
]

export const catalogContains = {
  id(id: string): boolean {
    return TTS_LOCAL_CATALOG.some((a) => a.id === id)
  },
}

export function expectedExtension(fileNameOrUrl: string): string {
  const trimmed = fileNameOrUrl.split('?')[0] ?? ''
  const lastDot = trimmed.lastIndexOf('.')
  if (lastDot < 0 || lastDot === trimmed.length - 1) return 'bin'
  return trimmed.slice(lastDot + 1).toLowerCase()
}
