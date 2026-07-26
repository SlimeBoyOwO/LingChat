import assert from 'node:assert/strict'

const catalogModule = await import('../src/api/services/tts-catalog.ts')
const { TTS_LOCAL_CATALOG, catalogContains, expectedExtension } = catalogModule

assert.equal(catalogContains.id('deberta'), true)
assert.equal(catalogContains.id('deberta-tokenizer'), true)
assert.equal(catalogContains.id('ling-v2'), true)
assert.equal(catalogContains.id('ling-v2-style'), true)

assert.equal(catalogContains.id('tsukuyomi'), false)
assert.equal(catalogContains.id('amitaro'), false)

assert.equal(expectedExtension('deberta.onnx'), 'onnx')
assert.equal(expectedExtension('tokenizer.json'), 'json')
assert.equal(expectedExtension('style_vectors.json'), 'json')
assert.equal(expectedExtension('sbv2api-model-Ling-v2-onnx.onnx'), 'onnx')

const deberta = TTS_LOCAL_CATALOG.find((a) => a.id === 'deberta')
assert.equal(deberta?.kind, 'bert')
assert.equal(
  deberta?.download_url,
  'https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/deberta.onnx',
)

const lingV2 = TTS_LOCAL_CATALOG.find((a) => a.id === 'ling-v2')
assert.equal(lingV2?.kind, 'voice')

const lingV2Style = TTS_LOCAL_CATALOG.find((a) => a.id === 'ling-v2-style')
assert.equal(lingV2Style?.kind, 'style_vectors')
assert.equal(lingV2Style?.voice_id, 'ling-v2')

console.log('TTS catalog tests passed')
