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
