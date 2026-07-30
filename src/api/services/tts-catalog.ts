export type AssetKind = 'bert' | 'voice' | 'style_vectors'

export interface CatalogAsset {
  id: string
  kind: AssetKind
  display_name: string
  language: string
  size_bytes: number
  download_url: string
  source: string
  voice_id?: string
  /** 下载此资产时自动连带下载的子资产 ID */
  bundled_assets?: string[]
}
