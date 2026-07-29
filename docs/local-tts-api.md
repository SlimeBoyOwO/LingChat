# LingChat 内置本地 TTS 接口文档

## 1. 概述

LingChat 的内置 TTS 基于 Style-Bert-VITS2（SBV2），以 Rust crate 的形式运行在 Tauri 主进程内：

- crate：`src-tauri/crates/sbv2-local-tts`
- 主程序桥接：`src-tauri/src/ai_service/tts/local`
- 前端封装：`src/api/services/tts-local.ts`
- 角色 TTS 类型：`localsbv2api`

它不是 HTTP 服务，不监听端口，也不使用 `sbv2-api` 的 Web API。前端通过 Tauri `invoke` 调用命令，主程序内部通过 Rust API 调用引擎。云端 TTS 仍由原有 Provider 处理。

模型不随安装包发布。调用合成接口前必须安装：

1. DeBERTa ONNX 模型；
2. DeBERTa Tokenizer；
3. 至少一个语音模型；
4. ONNX 语音模型对应的 `style_vectors.json`。

`.sbv2` 模型已内嵌风格向量，不需要单独的 `style_vectors.json`。

## 2. 文件目录

```text
<data_root>/
└── models/
    └── tts-local/
        ├── assets/
        │   └── deberta/
        │       ├── deberta.onnx
        │       └── tokenizer.json
        └── voices/
            └── <voice_id>/
                ├── model.onnx
                └── style_vectors.json
```

语音模型也可以使用：

```text
voices/<voice_id>/model.sbv2
```

平台路径规则：

| 平台 | `data_root` |
| --- | --- |
| Windows 开发环境 | 项目的 `data` 目录 |
| Windows 发布版 | 程序所在目录的 `data` 目录 |
| Android | 应用外部专属数据目录 |
| iOS | 应用沙箱内由主程序提供的目录 |

下载和导入过程的临时文件位于 Tauri `app_cache_dir/tts-local-cache`，不属于永久模型目录。

## 3. 前端调用入口

推荐通过现有 TypeScript 封装调用，不要在组件中重复手写命令名：

```ts
import * as TtsLocal from '@/api/services/tts-local'
```

Tauri 参数在 Rust 中使用 `snake_case`，前端 `invoke` 参数使用 `camelCase`。例如 Rust 参数 `voice_id` 在前端传为 `voiceId`。

所有命令失败时均以 rejected Promise 返回，错误值通常是后端生成的字符串：

```ts
try {
  const state = await TtsLocal.status()
  console.log(state)
} catch (error) {
  console.error('读取本地 TTS 状态失败', error)
}
```

## 4. 数据类型

### 4.1 `TtsLocalStatus`

```ts
interface TtsLocalStatus {
  ready: boolean
  deberta_installed: boolean
  installed_voice_count: number
}
```

| 字段 | 含义 |
| --- | --- |
| `ready` | DeBERTa Holder 是否已经初始化到内存 |
| `deberta_installed` | `deberta.onnx` 和 `tokenizer.json` 是否同时存在 |
| `installed_voice_count` | 检测到的语音目录数量 |

`deberta_installed: true` 不等于 `ready: true`。程序可能仍在后台初始化，或者初始化模型时发生错误。

### 4.2 `AssetRecord`

```ts
interface AssetRecord {
  asset_id: string
  kind: string
  size_bytes: number
  path: string
  language: string | null
  display_name: string | null
  source: string | null
}
```

当前共享资产列表只有完整的 DeBERTa 组合，`kind` 为 `bert`。只有模型和 Tokenizer 同时存在时才会出现在已安装列表中。

### 4.3 `VoiceRecord`

```ts
interface VoiceRecord {
  voice_id: string
  kind: 'onnx' | 'sbv2' | string
  size_bytes: number
  path: string
  language: string | null
  display_name: string | null
  source: string | null
  has_style_vectors: boolean
}
```

对于 `.sbv2`，`has_style_vectors` 始终为 `true`；对于 ONNX，该字段表示同目录是否存在 `style_vectors.json`。

### 4.4 `TtsLocalImportResult`

```ts
interface TtsLocalImportResult {
  asset_id: string
  voice_id: string | null
  path: string
  bytes: number
  message: string
}
```

`path` 是后端安装后的实际路径。共享资产的 `voice_id` 为 `null`。

### 4.5 `DownloadProgress`

```ts
interface DownloadProgress {
  asset_id: string
  bytes_done: number
  total_bytes: number
  percent: number
}
```

`percent` 当前为 `0` 至 `100`，不是 `0` 至 `1`。当服务器不返回 `Content-Length` 时，`total_bytes` 会使用目录中的预估大小。

## 5. Tauri Command

### 5.1 读取状态

命令：`tts_local_status`

封装：

```ts
const state = await TtsLocal.status()
```

原始调用：

```ts
const state = await invoke<TtsLocalStatus>('tts_local_status')
```

返回 `TtsLocalStatus`。

### 5.2 读取模型目录

命令：`tts_local_list_catalog`

```ts
const catalog = await invoke<AssetEntry[]>('tts_local_list_catalog')
```

后端返回内置下载目录，包括共享资产、语音模型和风格向量。`AssetEntry` 结构如下：

```ts
interface AssetEntry {
  id: string
  kind: 'bert' | 'voice' | 'style_vectors'
  display_name: string
  language: string
  size_bytes: number
  download_url: string
  source: string
  voice_id?: string
}
```

`TtsLocal.listCatalog()` 直接调用该命令；后端 `registry` 是唯一目录数据源。

### 5.3 列出已安装模型

命令：`tts_local_list_installed`

```ts
const snapshot = await TtsLocal.listInstalled()

for (const voice of snapshot.voices) {
  console.log(voice.voice_id, voice.kind, voice.has_style_vectors)
}
```

返回：

```ts
interface TtsLocalInstallSnapshot {
  assets: AssetRecord[]
  voices: VoiceRecord[]
}
```

该命令读取磁盘，不保证引擎已经把模型加载到内存。

### 5.4 从文件路径导入

命令：`tts_local_import_from_path`

封装：

```ts
TtsLocal.importFromPath(
  path: string,
  options?: {
    voiceId?: string
    assetId?: 'deberta' | 'deberta-tokenizer'
  },
): Promise<TtsLocalImportResult>
```

导入 DeBERTa：

```ts
await TtsLocal.importFromPath(selectedPath, { assetId: 'deberta' })
```

目标文件固定为：

```text
assets/deberta/deberta.onnx
```

导入 Tokenizer：

```ts
await TtsLocal.importFromPath(selectedPath, {
  assetId: 'deberta-tokenizer',
})
```

目标文件固定为：

```text
assets/deberta/tokenizer.json
```

导入语音模型：

```ts
await TtsLocal.importFromPath(selectedPath, { voiceId: 'ling-v2' })
```

不传 `voiceId` 时，后端根据源文件名生成 ID：转为小写，将非 ASCII 字母、数字、`-`、`_` 的字符替换为 `-`。无法生成有效名称时使用 `voice`。

语音导入支持当前归档检查器可识别的原始 `.onnx`、`.sbv2` 及支持的压缩包。Android 的 `content://` URI 会先通过 SAF 桥接复制到缓存，再进入同一安装流程。

不要同时传 `assetId` 和 `voiceId`。只要 `assetId` 非空，命令就按共享资产处理并忽略语音导入路径。

### 5.5 下载目录资产

命令：`tts_local_download`

```ts
const result = await TtsLocal.download('ling-v2')
```

`assetId` 必须存在于后端模型目录中。当前目录 ID 包括：

| ID | 类型 | 安装目标 |
| --- | --- | --- |
| `deberta` | `bert` | `assets/deberta/deberta.onnx` |
| `deberta-tokenizer` | `bert` | `assets/deberta/tokenizer.json` |
| `ling-v2` | `voice` | `voices/ling-v2/model.onnx` |
| `ling-v2-style` | `style_vectors` | `voices/ling-v2/style_vectors.json` |

下载器支持重定向，超时为 600 秒，并发送 `User-Agent: LingChat/0.4.6`。下载期间会先写 `.part` 临时文件，成功后再改名。当前不执行哈希校验。

当前没有公开的 `tts_local_cancel_download` command。后端存在取消令牌，但前端 API 暂时不能主动取消下载。

### 5.6 删除语音模型

命令：`tts_local_delete_voice`

```ts
await TtsLocal.deleteVoice('ling-v2')
```

删除整个 `voices/<voice_id>` 目录。删除不存在的语音视为成功。

`voiceId` 约束：

- 长度为 1 至 64；
- 只允许 ASCII 字母、数字、`-` 和 `_`；
- 后端会进行 canonical path 检查，拒绝目录穿越。

该命令不会删除 DeBERTa 或 Tokenizer，也不会显式清除引擎中的语音缓存。删除当前正在使用的模型后，后续重新加载会失败；建议删除前停止相应角色的语音生成。

### 5.7 导入风格向量

命令：`tts_local_import_style_vectors`

```ts
await TtsLocal.importStyleVectors('ling-v2', selectedJsonPath)
```

直接调用 Tauri IPC 时也统一使用 `voiceId`，不要传 Rust 内部参数名 `voice_id`：

```ts
await invoke('tts_local_import_style_vectors', {
  voiceId: 'ling-v2',
  path: selectedJsonPath,
})
```

前置条件：

- `voices/<voice_id>` 已存在；
- 先导入 ONNX 语音模型；
- 目标语音不能是 `.sbv2` 格式；
- `voiceId` 满足安全命名约束。

目标路径固定为：

```text
voices/<voice_id>/style_vectors.json
```

当前实现按文件复制，不在导入阶段验证 JSON schema；格式错误通常会在加载语音模型时暴露。

### 5.8 生成试听音频

命令：`tts_local_synthesize_preview`

```ts
const bytes = await TtsLocal.synthesizePreview({
  text: 'こんにちは。',
  voiceId: 'ling-v2',
  lengthScale: 1.0,
  sdpRatio: 0.0,
})

const wav = new Blob([new Uint8Array(bytes)], { type: 'audio/wav' })
const url = URL.createObjectURL(wav)
const audio = new Audio(url)
await audio.play()
audio.addEventListener('ended', () => URL.revokeObjectURL(url), { once: true })
```

参数：

| 参数 | 类型 | 含义 |
| --- | --- | --- |
| `text` | `string` | 待合成文本 |
| `voiceId` | `string` | 已安装语音 ID |
| `lengthScale` | `number` | 时长缩放，值越大语速越慢 |
| `sdpRatio` | `number` | SDP 噪声比例 |

试听接口当前固定：

- `style_id = 0`
- `speaker_id = 0`
- `style_weight = 1.0`
- `split_sentences = true`

返回值是 WAV 文件字节。后端使用 Tauri Raw IPC 响应，前端直接收到 `Uint8Array`。

试听要求引擎已经初始化。若 DeBERTa 文件刚导入，导入命令会尝试初始化；应用启动时若检测到文件，则在游戏主体初始化后后台预加载。

## 6. 事件

### 6.1 `tts://download-progress`

下载期间每间隔 200 ms 或新增 1 MiB 数据时发送一次；成功完成后会强制发送 `percent: 100` 的最终事件：

```ts
const unsubscribe = TtsLocal.onDownloadProgress((progress) => {
  console.log(
    progress.asset_id,
    progress.bytes_done,
    progress.total_bytes,
    progress.percent,
  )
})

// 组件销毁时解除订阅
unsubscribe()
```

也可以直接监听：

```ts
import { listen } from '@tauri-apps/api/event'

const unlisten = await listen<DownloadProgress>(
  'tts://download-progress',
  ({ payload }) => console.log(payload),
)
```

### 6.2 `tts://install-complete`

本地导入成功后发送。payload 为字符串：共享资产 ID 或语音 ID。

```ts
const unlisten = await listen<string>('tts://install-complete', ({ payload }) => {
  console.log('安装完成', payload)
})
```

### 6.3 `tts://download-complete`

下载命令退出时发送，payload 为请求的 `assetId`。

```ts
const unlisten = await listen<string>('tts://download-complete', ({ payload }) => {
  console.log('下载流程结束', payload)
})
```

注意：当前该事件在下载成功和失败后都会发送，因此它表示“流程结束”，不能作为成功依据。下载是否成功应以 `TtsLocal.download()` Promise 的 resolve/reject 为准。

### 6.4 `tts://engine-ready`

应用启动后的后台 DeBERTa 预加载成功时发送，payload 为空：

```ts
const unlisten = await listen('tts://engine-ready', () => {
  console.log('本地 TTS 引擎已就绪')
})
```

该事件只覆盖启动后台预加载。运行时导入 DeBERTa 后初始化成功，目前不会发送此事件；需要调用 `status()` 复查 `ready`。

## 7. 角色正式 TTS 配置

角色通过 `settings.yml` 选择本地 TTS：

```yaml
tts_type: localsbv2api
voice_lang: ja
voice_models:
  sbv2_local_voice_id: ling-v2
  sbv2_local_speaker_id: 0
  sbv2_local_style_id: 0
  sbv2_local_length_scale: 1.0
  sbv2_local_sdp_ratio: 0.0
  sbv2_local_cloud_fallback_model: Ling v2
  sbv2_local_cloud_fallback_speaker_id: '0'
```

字段说明：

| 字段 | 默认值 | 含义 |
| --- | --- | --- |
| `sbv2_local_voice_id` | 无 | 必填，映射到 `voices/<voice_id>` |
| `sbv2_local_speaker_id` | `0` | 说话人 ID |
| `sbv2_local_style_id` | `0` | 风格 ID |
| `sbv2_local_length_scale` | `1.0` | 时长缩放，越大越慢 |
| `sbv2_local_sdp_ratio` | `0.0` | SDP 噪声比例 |
| `sbv2_local_cloud_fallback_model` | 无 | 全局关闭本地 TTS 时使用的云端 SBV2 API 模型，可留空 |
| `sbv2_local_cloud_fallback_speaker_id` | 无 | 云端备用模型的说话人 ID，可留空 |

角色正式调用与试听独立：试听参数不会写回角色设置。角色设置保存后，已加载角色的 `VoiceMaker` 会重新构建并使用新的模型及参数。

全局本地 TTS 开关关闭时，配置为 `localsbv2api` 的角色会改走现有 `sbv2api` 云端流程。该回退依赖角色原有云端 SBV2 配置可用；关闭本地开关不代表任意角色都一定能够成功使用云端 TTS。

## 8. Rust crate API

主程序添加依赖：

```toml
[dependencies]
sbv2-local-tts = { path = "crates/sbv2-local-tts" }
```

crate 公开的主要类型：

```rust
pub use commands::LocalTtsState;
pub use engine::{LocalTtsEngine, SynthesizeRequest};
pub use paths::{LocalTtsPaths, VoiceInstallInfo, REQUIRED_ASSETS};
pub use registry::AssetEntry;
```

### 8.1 初始化路径和状态

```rust
use sbv2_local_tts::{LocalTtsPaths, LocalTtsState};

let paths = LocalTtsPaths::resolve(&app_handle, data_root)?;
paths.ensure()?;

let state = LocalTtsState::new(paths);
let engine = state.engine.clone();
```

`LocalTtsState` 同时保存路径、共享引擎和下载取消令牌。Tauri 主程序使用 `app.manage(state)` 将其注册为 command state。

### 8.2 初始化引擎

```rust
engine.init(&paths).await?;
assert!(engine.is_ready().await);
```

`init()` 从以下路径读取字节：

```text
assets/deberta/deberta.onnx
assets/deberta/tokenizer.json
```

随后创建 `sbv2_core::tts::TTSModelHolder`。模型构建在 `spawn_blocking` 中执行。

### 8.3 加载语音并合成

```rust
use sbv2_local_tts::SynthesizeRequest;

engine.load_voice(&paths, "ling-v2").await?;

let wav = engine
    .synthesize(SynthesizeRequest {
        voice_id: "ling-v2".to_owned(),
        text: "こんにちは。".to_owned(),
        style_id: 0,
        speaker_id: 0,
        sdp_ratio: 0.0,
        length_scale: 1.0,
    })
    .await?;

std::fs::write("preview.wav", wav)?;
```

`load_voice()` 优先查找 `model.sbv2`，其次查找 `model.onnx`。ONNX 形式缺少 `style_vectors.json` 时会直接失败。

引擎内部使用异步 Mutex 和串行锁保护 `TTSModelHolder`。`load_voice()` 与 `synthesize()` 不应被外部再包一层自定义并发访问逻辑；当前引擎会自动串行化 ONNX Holder 操作。

## 9. 初始化时序

应用启动流程：

1. 主程序初始化 `data` 路径；
2. 解析并创建本地 TTS 目录；
3. 创建 `LocalTtsState` 并注册 Tauri state；
4. 优先初始化数据库、游戏服务和角色等主体内容；
5. 若 DeBERTa 与 Tokenizer 已安装，则后台执行 `engine.init()`；
6. 成功后发送 `tts://engine-ready`；
7. 首次正式角色合成时，适配器再次检查引擎状态，并按需初始化、加载目标语音；
8. 合成调用通过串行锁执行，避免多个文本片段同时取走 Holder 后出现 `engine not initialized`。

因此，前端不应在应用刚打开时假设 `ready` 立即为 `true`。需要试听时可以禁用按钮并监听 `tts://engine-ready`，或定期调用 `status()`。

## 10. 常见错误

### `local TTS engine not initialized (missing DeBerta)`

原因：试听前引擎未初始化。

检查：

- `assets/deberta/deberta.onnx` 是否存在；
- `assets/deberta/tokenizer.json` 是否存在；
- `status().deberta_installed` 与 `status().ready`；
- 后端日志中是否有 `TTSModelHolder::new` 错误。

### `engine not initialized`

原因：直接调用 `load_voice()` 或 `synthesize()` 前未成功执行 `init()`，或者 DeBERTa 初始化失败。

### `voice <id> not installed`

原因：对应目录中没有 `model.sbv2` 或 `model.onnx`。

### `voice <id> is missing style_vectors.json required by model.onnx`

原因：语音是 ONNX 格式，但未导入风格向量。

### `unknown package format`

原因：语音导入路径进入了归档检查器，但文件既不是受支持的原始模型，也不是可识别压缩包。检查文件扩展名、文件头和 Android SAF 复制结果。

共享 DeBERTa 和 Tokenizer 不应按语音模型导入，必须传对应的 `assetId`。

### `voice id must be kebab-case ASCII`

原因：语音 ID 含有空格、斜杠、中文或其他不允许字符。使用类似 `ling-v2` 或 `voice_01` 的 ID。

### `HTTP 403 Forbidden`

原因：模型托管服务拒绝请求、链接权限不足或链接已经失效。下载器会返回最终重定向 URL 和最多 512 字符的响应正文，优先根据该信息排查。

### `com.microsoft.Gelu` 不支持 `tensor(float16)`

原因：当前 CPU Execution Provider 无法执行该 FP16 自定义算子。应使用兼容 CPU Provider 的 FP32 DeBERTa ONNX 模型，或者在未来明确接入支持该模型的执行提供器。

## 11. 当前接口边界

以下能力当前未公开或尚未实现：

- 没有本地 TTS HTTP API；
- 没有公开下载取消 command；
- 没有删除共享 DeBERTa/Tokenizer 的 command；
- 试听接口不能指定 `speakerId` 或 `styleId`；
- 导入 `style_vectors.json` 时不验证 JSON schema；
- `download-complete` 不区分成功与失败；
- `listCatalog()` 前端封装与后端 command 使用两个目录来源；
- 引擎没有公开卸载单个语音或清空内存缓存的 command。

调用方不应假设这些能力已经存在。扩展接口时需要同步更新 Rust command、`src-tauri/src/lib.rs` 的 invoke handler、TypeScript 封装及本文档。
