# 文本嵌入模型（Tool RAG）

`bge-small-zh-v1.5` 的 uint8 量化 ONNX 导出，供主聊天的 Tool RAG 做工具语义检索
（见 `docs/function_call/tool_rag.md`）。512 维，用于把工具描述与用户的自然语言
意图做向量匹配，检索 top-5 候选工具。

## 来源

- 模型：`BAAI/bge-small-zh-v1.5`（中文检索，512 维）
- 导出：Xenova 的 ONNX 转换仓（`onnx/model_uint8.onnx`）
- 下载源：ModelScope 镜像 `Xenova/bge-small-zh-v1.5`

## 获取方式

```
node scripts/download_embedding_model.mjs
```

`pnpm run init` 已包含这一步（`release` CI 会执行，`dev-build` CI 不执行）。

## 目录内容

本目录下的模型文件**不进 git**（`.gitignore` 的 `data/**` 排除整个 `data` 目录，
只有显式的 `!` 白名单路径会入库），仅 `.gitkeep` 与本文档受版本控制。

| 文件             | 说明                                                                            |
| ---------------- | ------------------------------------------------------------------------------- |
| `model.onnx`     | uint8 动态量化模型，单文件（无外部 `.data` 边文件，否则会被整目录打包进安装包） |
| `tokenizer.json` | HuggingFace 分词器（BERT WordPiece）                                            |
| `config.json`    | 仅供下载脚本做版本比对                                                          |

## 换模型

`crates/ling-chat-main/src/ai_service/embedding/engine.rs` 的 `MODEL_DIR_NAME`
与下载脚本的 `BASE_URL` / `FILES` 是仅有的两处需要同步修改的地方。换用别的模型
（例如多语言模型）时注意：必须是单文件 ONNX，否则安装包会被外部边文件撑大。
