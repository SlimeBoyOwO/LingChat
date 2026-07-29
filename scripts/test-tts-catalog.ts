import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const catalogSource = await readFile(
  new URL('../src/api/services/tts-catalog.ts', import.meta.url),
  'utf8',
)
const serviceSource = await readFile(
  new URL('../src/api/services/tts-local.ts', import.meta.url),
  'utf8',
)

assert.doesNotMatch(catalogSource, /TTS_LOCAL_CATALOG/)
assert.doesNotMatch(catalogSource, /modelscope\.cn/)
assert.doesNotMatch(catalogSource, /catalogContains/)

assert.match(
  serviceSource,
  /invoke<readonly CatalogAsset\[\]>\('tts_local_list_catalog'\)/,
)
assert.doesNotMatch(serviceSource, /export const CATALOG/)

console.log('TTS catalog single-source tests passed')
