# bge-small-zh-v1.5（可选本地中文量化版）

中文嵌入模型 **BAAI/bge-small-zh-v1.5** 的本地量化产物（约 11MB），
无需联网 / 无 API Key。

> **注意**：这些二进制产物**不再随仓库 / 应用分发**，本目录只保留说明。
> 未提供模型文件、且未配置远程 `embedding.*` 时，长期记忆会自动退化为
> **字符 n-gram 词法检索**（非嵌入式 RAG），功能仍可用（见
> `crates/ling-chat-main/src/ai_service/memory/search.rs`）。
> 若要启用本地语义检索，最省事的方式是在**设置页 → 功能设置 → 嵌入模型 →
> 「本地嵌入模型」面板**里从本地文件夹或 zip / 7z 压缩包导入下列四个产物
> （导入后立即生效，无需重启）；也可以手动把产物放回本目录
> （运行时路径为 `data_dir/third_party/bge_small_zh_v1_5/`，可用
> `scripts/quantize_bge_embedding.py` 从原始 bge 模型重新生成）。

## 产物

| 文件                 | 大小  | 说明                                                                                                       |
| -------------------- | ----- | ---------------------------------------------------------------------------------------------------------- |
| `model_int4.onnx`    | 7.9MB | Transformer 全部 MatMul int4（MatMulNBits 节点），词嵌入表已从图中摘除，改为外部 fp32 输入 `inputs_embeds` |
| `embedding_int2.bin` | 3.4MB | 21128×512 词嵌入表：int2 + 每行 8 组（64 维/组）× fp16(scale,zero) 仿射量化，按行主序 LSB-first 打包       |
| `tokenizer.json.gz`  | 0.1MB | BERT WordPiece 词表（vocab 21128），gzip 压缩，加载时解压                                                  |
| `config.json`        | 0.2KB | 模型元信息（512 维 / CLS 池化 / 512 上下文）                                                               |

格式与解码约定见 `crates/ling-chat-main/src/ai_service/memory/local_embedding.rs`，
复现脚本 `scripts/quantize_bge_embedding.py`（图手术 → int4 → int2 打包 → tokenizer）。

## 校验

端到端（词嵌入表只有 2-bit）：与原始 fp32 参考相比，查询向量余弦约 0.91；
`model_int4` 单独（词表 fp32）余弦约 0.95。运行环境加载失败时会优雅降级为「未配置」，
不影响其余功能。

## 与旧模型（bekko-embedding-v1-a25m）的差异

- 词表 256000 → 21128、维度 384 → 512，体积 49MB → 11MB；
- 池化由带 mask 的均值池化改为 **CLS（序列首位）池化**（bge 官方池化方式）；
- 上下文上限 8192 → 512；
- 因维度/语义变化，旧向量按 `model` 列过滤后自动失效（不参与检索）。
