# LingChat 记忆嵌入（Embedding）说明

嵌入为记忆系统提供 **语义检索** 与 **去重** 两种能力：

- **语义检索**：运行时把记忆库（MemoryBank）各段 + 手动笔记编入向量索引，
  按当前对话语义召回最相关的片段，作为「【语义召回的记忆】」注入 LLM 上下文，
  让 AI 能精确想起过去聊过的某件事，而不只是把整库文本强行塞进去。
  同时 `memory_search` 工具供 AI 主动按语义查询某段记忆。
- **去重**：新增手动笔记时，若与既有记忆语义高度相似（余弦 ≥ 0.88）则拒绝写入，
  避免记忆库膨胀出同义重复条目；索引刷新时也自动跳过重复片段。

## 架构

```
┌────────────────────────────────────────────┐
│  Rust (EmbeddingManager)                    │
│  ai_service/embedding/                      │
│  tokenizer.rs: WordPiece / SentencePiece    │
│  service.rs:   ort (ONNX) + 前缀 + pooling  │
│  memory_index.rs: 内存向量索引 + 检索 + 去重   │
└────────────────────────────────────────────┘
```

- **推理在 Rust 侧原生完成**：`ort`（ONNX Runtime）直接加载 `model.onnx`，无需 Python。
- **分词**：`tokenizer.rs` 同时支持 WordPiece（BERT 系）与 SentencePiece Unigram
  （XLM-RoBERTa 系），按 `tokenizer.json` 的 `model.type` 自动分发。
- **向量**：逐条推理 → attention-mask 加权 mean pooling → L2 归一化。
- **前缀**：内置模型（paraphrase-multilingual-MiniLM）不区分配置 `query: ` /
  `passage: ` 前缀（默认留空）；如将来换用 E5 系模型，可在配置中启用前缀，
  索引仍存储原始文本（不含前缀）。
- **记忆索引**：内存向量索引 + 余弦检索 + 去重阈值判断。
- **集成点**：`memory_index.rs` 仍负责笔记去重（`memory_add_note`）与
  `memory_search` 主动检索工具；**自动召回**改为使用独立语义记忆库
  （见下文「独立语义记忆」）。

服务运行时不依赖任何外部进程；模型目录与分词器全部在 Rust 侧载入。

## 独立语义记忆（Semantic Memory）

> 自 0.5.0 起，**自动语义召回** 与普通记忆库（MemoryBank）/笔记**完全解耦**，
> 由一套独立系统承载：

- **专属工具**：`semantic_mem_add` / `semantic_mem_search` / `semantic_mem_delete` /
  `semantic_mem_list` 由 AI 直接写/查专属库，与普通记忆系统互不读取。
- **专属向量库**：SQLite（`data/game_data/semantic_memory.db`），表 `semantic_memory`
  （`id` / `role_id` / `text` / `vector` BLOB(LE f32) / `tags` JSON / 时间戳），
  按 `role_id` 分库，Rust 暴力余弦（向量已 L2 归一化，余弦=点积）检索。
- **召回注入**：`GameRoleManager::build_semantic_recall` 每轮查询该库，
  把命中片段作为「【语义召回的记忆】」注入 LLM 上下文；**不再**依赖
  MemoryBank/笔记，即使普通记忆关闭也照常工作。
- **开关**：`semantic_memory.enabled`（默认 false，重启生效）+ 嵌入引擎就绪
  才可编码/检索。
- **参数**：去重 0.88 / 召回过滤 0.35 / top_k 默认 4（clamp 1..=20）。
- **使用限制**：新工具默认未授权，需先在「工具管理」页批准 `semantic_mem_*`。

在应用内「高级设置 → 语义记忆」分类可查看运行状态（向量库位置 / 是否就绪 /
记忆条数）并配置开关与 top_k。

## 模型选择（多语言 · 越小越好）

默认模型 `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2`：

| 项目 | 值 |
| ---- | ---- |
| 架构 | BERT 系 MiniLM（hidden=384，layers=12） |
| 维度 | 384 |
| 语言 | 多语言（含中文 / 英文 / 日语，50+ 语种对架构） |
| 分词器 | SentencePiece Unigram（`tokenizer.json`，`model.type: "Unigram"`，250k XLM-R 词表） |
| Pooling | mean（与 sentence-transformers `1_Pooling` 打包一致） |
| 前缀 | 无需（默认留空；保留配置项以兼容 E5 系模型） |
| int4 量化体积 | ~68 MB（450MB float32） |

Rust 侧只通过 `ort` 直跑目录中的 `model.onnx`（`model_quantized.onnx` 优先）。
标准 sentence-transformers 目录若缺少 ONNX 导出文件，需先用 `export_onnx.py` 转换；
`st` 后端（torch）不再支持。模型导出时 batch 固定为 1，运行时逐条推理即可。

## 安装步骤

1. 准备模型（需含 `model_quantized.onnx` 或 `model.onnx` + `tokenizer.json`）：

```bash
# 在中国大陆网络建议用镜像（脚本和 snapshot_download 都会读 HF_ENDPOINT）：
export HF_ENDPOINT=https://hf-mirror.com

bash scripts/embedding/setup.sh
```

脚本默认下载 `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2`，
转换并 int4 量化到 `data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2/`。
（转换脚本 `scripts/embedding/export_onnx.py`；量化 `scripts/embedding/quantize_onnx.py`。）

2. 配置 `settings.json` 开启嵌入（键位见 `src-tauri/src/config/keys.rs`）：

```jsonc
{
  "embedding.enabled": true,
  "embedding.model_dir": "/home/user/data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2",
  "embedding.backend": "auto",
  "embedding.query_prefix": "",
  "embedding.passage_prefix": ""
}
```

- `embedding.model_dir` 为空时默认 `data/third_party/embedding/`；打包版会回退到
  资源目录内的内置模型。
- `embedding.query_prefix` / `embedding.passage_prefix` 默认**均为空**（内置
  MiniLM 模型无需前缀）；若换用 E5 等带 `query: `/`passage: ` 训练的模型，
  可在设置或 `settings.json` 中填上相应前缀字符串。
- 未配置 / 模型缺失 / ONNX 加载失败时功能自动禁用并记录日志，不影响对话。
- `embedding.python` 为旧版遗留字段，已不再使用（保留仅为了兼容旧配置文件）。

若要启用独立语义记忆（自动召回），在配置中追加 `semantic_memory` 组：

```jsonc
{
  "semantic_memory.enabled": true,
  "semantic_memory.top_k": 4
}
```

3. 重启应用生效（配置在启动时读取）。

也可以在应用内「高级设置 → 记忆嵌入」分类中直接配置（对应
`src-tauri/src/config/tree.rs` 的配置树），保存后重启生效。

## 打包分发（自带模型，无 Python）

分发安装包默认**已内置模型**
（`data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2/`），
安装后开箱即用，无需 Python：

- 构建流程：`beforeBuildCommand` 运行 `prepare-desktop-resources.mjs`，把 `data/`
  下的 game_data 与 `data/third_party/`（含上述内置模型目录）打进资源目录。
- 运行时回退：应用启动时若 `embedding.model_dir` 未配置，`to_service_config`
  （`src-tauri/src/config/embedding.rs`）优先使用资源目录内置模型，
  否则数据目录默认路径。
- 推理全部在 Rust 侧（`ort`）完成，不引入任何 Python / 子进程 / 解释器依赖。

## 调试

- 日志关键词：`[embedding]`。
- 端到端集成测试（需自备模型）：

```bash
EMBEDDING_TEST_MODEL=data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2 \
  cargo test --lib encodes_through_onnx_session
```

连接模型做召回/去重的行为可用同一个模型目录在测试 `embedding` 模块全量覆盖。
