import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const read = (path: string) =>
  readFile(new URL(`../${path}`, import.meta.url), 'utf8')

const [voiceMaker, types, commands, engine, paths, registry, download] = await Promise.all([
  read('src-tauri/src/ai_service/tts/voice_maker.rs'),
  read('src-tauri/src/ai_service/types.rs'),
  read('src-tauri/crates/sbv2-local-tts/src/commands.rs'),
  read('src-tauri/crates/sbv2-local-tts/src/engine.rs'),
  read('src-tauri/crates/sbv2-local-tts/src/paths.rs'),
  read('src-tauri/crates/sbv2-local-tts/src/registry.rs'),
  read('src-tauri/crates/sbv2-local-tts/src/download.rs'),
])

const localBranch = voiceMaker.match(
  /"localsbv2api" if self.availability.sbv2_local => {([\s\S]*?)\n            }/,
)?.[1] ?? ''
assert.ok(localBranch, 'localsbv2api branch must exist')
assert.doesNotMatch(localBranch, /sbv2api_name|sbv2api_speaker_id/)
assert.doesNotMatch(localBranch, /Sbv2ApiAdapter::new/)

assert.match(types, /sbv2_local_cloud_fallback_model: Option<String>/)
assert.match(types, /sbv2_local_cloud_fallback_speaker_id: Option<String>/)
assert.match(voiceMaker, /sbv2_local_cloud_fallback_model/)
assert.match(voiceMaker, /sbv2_local_cloud_fallback_speaker_id/)

assert.match(
  commands,
  /let deberta_installed = state\.paths\.asset_present\("deberta"\);/,
)
assert.doesNotMatch(engine, /voice_cache|HashMap/)
assert.doesNotMatch(paths, /#!\[allow\(dead_code\)\]/)
assert.doesNotMatch(registry, /sha256/i)
assert.doesNotMatch(download, /sha256|Sha256/)

console.log('TTS architecture tests passed')
