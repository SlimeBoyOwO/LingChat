# bekko-embedding-v1-a25m（内置量化版）

现代 BERT 嵌入模型 **hotchpotch/bekko-embedding-v1-a25m** 的本地量化产物，
随应用分发（约 49MB），无需联网 / 无 API Key，开箱即用。

## 产物

| 文件                 | 大小   | 说明                                                                                                       |
| -------------------- | ------ | ---------------------------------------------------------------------------------------------------------- |
| `model_int4.onnx`    | 13.5MB | Transformer 全部 MatMul int4（MatMulNBits 节点），词嵌入表已从图中摘除，改为外部 fp32 输入 `inputs_embeds` |
| `embedding_int2.bin` | 30.7MB | 256000×384 词嵌入表：int2 + 每行 6 组（64 维/组）× fp16(scale,zero) 仿射量化，按行主序 LSB-first 打包      |
| `tokenizer.json.gz`  | 4.8MB  | BPE 词表（vocab 256000 / merges 580604），gzip 压缩，加载时解压                                            |
| `config.json`        | 1.7KB  | 模型元信息（384 维 / 均值池化 / 8192 上下文）                                                              |

格式与解码约定见 `crates/ling-chat-main/src/ai_service/memory/local_embedding.rs`，
复现脚本 `scripts/quantize_bekko_embedding.py`（图手术 → int4 → int2 打包 → tokenizer）。

## 校验

端到端（词嵌入表只有 2-bit）：与原始 fp32 参考相比，查询向量余弦 0.90–0.93；
检索排序（正 / 负样本对）全部正确。运行环境加载失败时会优雅降级为「未配置」，
不影响其余功能。
