import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const read = (path: string) =>
  readFile(new URL(`../${path}`, import.meta.url), 'utf8')

const [lib, download, adapter, voiceMaker, settingsTts, characterSettings, apiDocs] =
  await Promise.all([
    read('src-tauri/src/lib.rs'),
    read('src-tauri/crates/sbv2-local-tts/src/download.rs'),
    read('src-tauri/src/ai_service/tts/local/adapter.rs'),
    read('src-tauri/src/ai_service/tts/voice_maker.rs'),
    read('src/components/settings/pages/SettingsTts.vue'),
    read('src/components/settings/pages/SettingsCharacterInfo.vue'),
    read('docs/local-tts-api.md'),
  ])

assert.match(
  lib,
  /timeout\(\s*Duration::from_secs\(15\),\s*preload_engine\.init\(&preload_paths\),?\s*\)/,
  'local TTS preload must time out after 15 seconds',
)

assert.match(download, /PROGRESS_EMIT_INTERVAL[^\n]*Duration::from_millis\(200\)/)
assert.match(download, /PROGRESS_EMIT_BYTES[^\n]*1024 \* 1024/)
assert.match(
  download,
  /emit_download_progress\([^;]*bytes_done[^;]*true/s,
  'download completion must force a final progress event',
)

assert.match(settingsTts, /tts:\/\/install-complete/)
assert.match(settingsTts, /tts:\/\/download-complete/)
assert.match(settingsTts, /unlistenInstallComplete/)
assert.match(settingsTts, /unlistenDownloadComplete/)
assert.match(
  settingsTts,
  /await refreshAll\(\)\s*if \(!componentMounted\) return\s*unlistenProgress = TtsLocal\.onDownloadProgress/,
  'download progress listener must not be registered after component unmount',
)

assert.match(
  adapter,
  /voice_id: String,\s*style_id: i32,\s*speaker_id: i64,\s*sdp_ratio: f32,\s*length_scale: f32,/,
  'LocalTtsAdapter parameters must follow SynthesizeRequest order',
)
assert.match(
  voiceMaker,
  /voice_id,\s*style_id,\s*speaker_id,\s*sdp_ratio,\s*length_scale,\s*paths,/,
  'LocalTtsAdapter call site must follow SynthesizeRequest order',
)

assert.match(characterSettings, /REALTIME_SAVE_DEBOUNCE_MS\s*=\s*300/)
assert.match(characterSettings, /clearRealtimeSaveTimer/)

assert.match(apiDocs, /`voiceId`/)
assert.match(
  apiDocs,
  /invoke\('tts_local_import_style_vectors',[\s\S]{0,180}voiceId/,
  'style-vector IPC request documentation must use voiceId',
)

console.log('TTS minor-fix contract tests passed')
