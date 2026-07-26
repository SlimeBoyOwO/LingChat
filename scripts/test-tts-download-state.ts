import assert from 'node:assert/strict'
const mod = await import('../src/utils/tts-download-state.ts')
const { catalogRowState } = mod
const catalog = (await import('../src/api/services/tts-catalog.ts')).TTS_LOCAL_CATALOG
const find = (id: string) => {
  const asset = catalog.find((a) => a.id === id)
  if (!asset) throw new Error(`missing fixture: ${id}`)
  return asset
}
const deberta = find('deberta')
const tokenizer = find('deberta-tokenizer')
const lingV2 = find('ling-v2')
const lingV2Style = find('ling-v2-style')
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
