import assert from 'node:assert/strict'
import type { CatalogAsset } from '../src/api/services/tts-catalog.ts'

const mod = await import('../src/utils/tts-download-state.ts')
const { catalogRowState } = mod

const asset = (
  id: string,
  kind: CatalogAsset['kind'],
  voiceId?: string,
): CatalogAsset => ({
  id,
  kind,
  display_name: id,
  language: 'ja',
  size_bytes: 0,
  download_url: 'https://example.invalid/model',
  source: 'test',
  voice_id: voiceId,
})

const deberta = asset('deberta', 'bert')
const tokenizer = asset('deberta-tokenizer', 'bert')
const lingV2 = asset('ling-v2', 'voice')
const lingV2Style = asset('ling-v2-style', 'style_vectors', 'ling-v2')
assert.equal(
  catalogRowState({ asset: deberta, status: { deberta_installed: true } }),
  'installed',
)
assert.equal(
  catalogRowState({ asset: deberta, status: { deberta_installed: false } }),
  'missing',
)
assert.equal(
  catalogRowState({ asset: tokenizer, status: { deberta_installed: true } }),
  'installed',
)
assert.equal(
  catalogRowState({ asset: tokenizer, status: { deberta_installed: false } }),
  'missing',
)
assert.equal(
  catalogRowState({ asset: lingV2, voices: [{ voice_id: 'ling-v2', has_style_vectors: false }] }),
  'installed',
)
assert.equal(catalogRowState({ asset: lingV2 }), 'missing')
assert.equal(
  catalogRowState({
    asset: lingV2Style,
    voices: [{ voice_id: 'ling-v2', has_style_vectors: true }],
  }),
  'installed',
)
assert.equal(
  catalogRowState({
    asset: lingV2Style,
    voices: [{ voice_id: 'ling-v2', has_style_vectors: false }],
  }),
  'missing',
)
assert.equal(
  catalogRowState({
    asset: lingV2,
    progressPercent: 42,
    voices: [{ voice_id: 'ling-v2', has_style_vectors: false }],
  }),
  'downloading',
)
assert.equal(
  catalogRowState({
    asset: lingV2,
    progressPercent: 100,
    errorMessage: 'network down',
  }),
  'error',
)
assert.equal(
  catalogRowState({
    asset: deberta,
    progressPercent: 100,
    status: { deberta_installed: false },
  }),
  'missing',
)
assert.equal(
  catalogRowState({
    asset: deberta,
    errorMessage: 'network down',
    status: { deberta_installed: true },
  }),
  'error',
)
console.log('TTS download state tests passed')
