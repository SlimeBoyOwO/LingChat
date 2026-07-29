import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const [engine, apiDocs] = await Promise.all([
  readFile(
    new URL('../src-tauri/crates/sbv2-local-tts/src/engine.rs', import.meta.url),
    'utf8',
  ),
  readFile(new URL('../docs/local-tts-api.md', import.meta.url), 'utf8'),
])

assert.match(engine, /SBV2_FIXTURE_DIR/)
assert.match(engine, /SBV2_FIXTURE_VOICE_ID/)
assert.match(engine, /fixture_happy_path_init_load_synthesize/)
assert.match(engine, /engine\s*\.init\(&paths\)\s*\.await/)
assert.match(engine, /engine\s*\.load_voice\(&paths, &voice_id\)\s*\.await/)
assert.match(engine, /engine\s*\.synthesize\(SynthesizeRequest/)
assert.match(engine, /assert_eq!\(&wav\[\.\.4\], b"RIFF"\)/)
assert.match(engine, /assert_eq!\(&wav\[8\.\.12\], b"WAVE"\)/)
assert.match(apiDocs, /SBV2_FIXTURE_DIR/)
assert.match(apiDocs, /SBV2_FIXTURE_VOICE_ID/)
assert.match(apiDocs, /fixture_happy_path_init_load_synthesize/)
assert.doesNotMatch(apiDocs, /两个目录来源/)

console.log('TTS fixture happy-path contract tests passed')
