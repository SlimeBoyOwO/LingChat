import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const lib = await readFile(
  new URL('../src-tauri/src/lib.rs', import.meta.url),
  'utf8',
)

assert.match(lib, /use tauri_plugin_dialog::\{DialogExt, MessageDialogKind\};/)
assert.match(lib, /let local_tts_paths_available = match tts_paths\.ensure\(\)/)
assert.match(lib, /\.kind\(MessageDialogKind::Error\)/)
assert.match(lib, /\.message\([\s\S]{0,300}无法创建本地 TTS 数据目录/)
assert.match(
  lib,
  /LocalTtsSwitch::new\(\s*local_tts_paths_available\s*&&\s*load_enable_local_tts/,
  'failed path initialization must disable local TTS for this process',
)
assert.match(
  lib,
  /tauri::async_runtime::spawn\(async move \{\s*tokio::task::yield_now\(\)\.await;/,
  'background preload must yield once before starting model work',
)

console.log('TTS optimization contract tests passed')
