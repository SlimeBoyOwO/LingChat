import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const read = (path: string) =>
  readFile(new URL(`../${path}`, import.meta.url), 'utf8')

const [
  localBridge,
  keys,
  lib,
  service,
  settingsTts,
  apiDocs,
  permissions,
  capability,
] = await Promise.all([
  read('src-tauri/src/ai_service/tts/local/mod.rs'),
  read('src-tauri/src/config/keys.rs'),
  read('src-tauri/src/lib.rs'),
  read('src/api/services/tts-local.ts'),
  read('src/components/settings/pages/SettingsTts.vue'),
  read('docs/local-tts-api.md'),
  read('src-tauri/permissions/tts-local.toml'),
  read('src-tauri/capabilities/default.json'),
])

assert.match(keys, /pub const ENABLE_LOCAL_TTS: &str = "features\.enable_local_tts";/)
assert.match(localBridge, /pub fn tts_local_get_enabled/)
assert.match(localBridge, /pub fn tts_local_set_enabled/)
assert.match(
  localBridge,
  /store\.save\(\)[\s\S]{0,300}switch\.set_enabled\(enabled\)/,
  'runtime switch must only change after settings are persisted',
)
assert.match(lib, /ai_service::tts::local::tts_local_get_enabled/)
assert.match(lib, /ai_service::tts::local::tts_local_set_enabled/)
assert.match(service, /invoke<LocalTtsSwitchStatus>\('tts_local_get_enabled'\)/)
assert.match(service, /invoke<LocalTtsSwitchStatus>\('tts_local_set_enabled', \{ enabled \}\)/)
assert.doesNotMatch(settingsTts, /getEnvConfigByKey\('features\.enable_local_tts'\)/)
assert.doesNotMatch(settingsTts, /saveEnvConfigSettings\(\{[\s\S]{0,100}features\.enable_local_tts/)
assert.match(apiDocs, /`tts_local_get_enabled`、`tts_local_set_enabled`/)
assert.match(apiDocs, /configured_enabled[\s\S]{0,100}effective_enabled/)
assert.match(
  permissions,
  /identifier = "tts-local-get-enabled"[\s\S]{0,200}commands\.allow = \["tts_local_get_enabled"\]/,
)
assert.match(
  permissions,
  /identifier = "tts-local-set-enabled"[\s\S]{0,200}commands\.allow = \["tts_local_set_enabled"\]/,
)
assert.match(capability, /"tts-local-get-enabled"/)
assert.match(capability, /"tts-local-set-enabled"/)

console.log('TTS switch IPC tests passed')
