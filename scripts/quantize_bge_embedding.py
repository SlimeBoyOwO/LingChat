#!/usr/bin/env python3
"""量化 bge-small-zh-v1.5 → 可选本地嵌入模型产物。

产物**不再随仓库/应用分发**（默认走字符 n-gram 词法检索）；如需本地语义检索，
把下列产物放回 `data_dir/third_party/bge_small_zh_v1_5/`（仓库同路径仅保留 README）。
复现后合计约 11MB：
    model_int4.onnx       7.9MB  Transformer 全部 MatMul int4（MatMulNBits），
                                  词嵌入表已从图中摘除，改为外部 fp32 输入 `inputs_embeds`
    embedding_int2.bin    3.4MB  21128×512 词嵌入表 int2 + 每行 8 组 × fp16(scale,zero)
    tokenizer.json.gz     0.1MB  BPE 词表 gzip
    config.json           参考元信息（512 维 / CLS 池化 / 512 上下文）

与 `quantize_bekko_embedding.py` 同套路（词嵌入外置 + int4 transformer + int2 打包 +
tokenizer gzip），只是源模型换成结构更简单的标准 BERT（BAAI/bge-small-zh-v1.5）：
词表从 bekko 的 256000 缩到 21128，词嵌入表因此从 30.7MB 降到 3.4MB，整体 ~11MB。

用法（Python 3.10+；onnx / onnxruntime / onnx_ir / numpy / tokenizers）：
    uv venv /tmp/bge-venv --python 3.12
    uv pip install --python /tmp/bge-venv/bin/python onnx onnxruntime onnx_ir numpy tokenizers
    # 原始 fp32 ONNX（Xenova 导出，含词表内嵌）与 tokenizer：
    curl -L -o /tmp/bge/model.onnx      https://hf-mirror.com/Xenova/bge-small-zh-v1.5/resolve/main/onnx/model.onnx
    curl -L -o /tmp/bge/tokenizer.json  https://hf-mirror.com/Xenova/bge-small-zh-v1.5/resolve/main/tokenizer.json
    BGE_SRC=/tmp/bge/model.onnx BGE_TOKENIZER=/tmp/bge/tokenizer.json python quantize_bge_embedding.py all
"""
import gzip
import json
import os
import shutil
import sys

import numpy as np
import onnx
from onnx import TensorProto, helper

SRC = os.environ.get("BGE_SRC", "/tmp/opencode/bge/model.onnx")
TOKENIZER_SRC = os.environ.get("BGE_TOKENIZER", "/tmp/opencode/bge/tokenizer.json")
OUT = os.environ.get(
    "BGE_OUT",
    os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        "data",
        "third_party",
        "bge_small_zh_v1_5",
    ),
)
os.makedirs(OUT, exist_ok=True)

EMBED_IN = "inputs_embeds"
EMBED_DIM = 512
VOCAB = 21128
WORD_INIT = "embeddings.word_embeddings.weight"
GATHER_NODE = "/embeddings/word_embeddings/Gather"
GATHER_OUT = "/embeddings/word_embeddings/Gather_output_0"


# ---------- 1. 图手术：词嵌入外置 ----------
def surgery():
    m = onnx.load(SRC, load_external_data=False)
    g = m.graph
    init = {t.name: t for t in g.initializer}
    w = onnx.numpy_helper.to_array(init[WORD_INIT])
    assert w.shape == (VOCAB, EMBED_DIM), w.shape
    w.astype(np.float32).tofile(os.path.join(OUT, "word_emb_fp32.bin"))
    print("word embedding:", w.shape)

    # 新增 fp32 输入 inputs_embeds，把词嵌入 Gather 的消费者改接到它。
    g.input.append(
        helper.make_tensor_value_info(
            EMBED_IN, TensorProto.FLOAT, ["batch_size", "sequence_length", EMBED_DIM]
        )
    )
    hits = 0
    for n in g.node:
        for i, name in enumerate(n.input):
            if name == GATHER_OUT:
                n.input[i] = EMBED_IN
                hits += 1
    assert hits >= 1, "no consumers of word embedding Gather"
    print("rewired consumers:", hits)

    keep = [n for n in g.node if n.name != GATHER_NODE]
    del g.node[:]
    g.node.extend(keep)

    # 删掉不再引用的初始化器（词嵌入表）。
    used = set()
    for n in g.node:
        used.update(n.input)
    keep_init = [t for t in g.initializer if t.name in used]
    removed = [t.name for t in g.initializer if t.name not in used]
    print("removed initializers:", removed)
    del g.initializer[:]
    g.initializer.extend(keep_init)

    onnx.checker.check_model(m, full_check=False)
    base = os.path.join(OUT, "base_fp32.onnx")
    onnx.save(m, base)
    print(f"base_fp32.onnx: {os.path.getsize(base)/1e6:.1f} MB")
    return base


# ---------- 2. int4 量化 ----------
def quantize_int4(base):
    from onnxruntime.quantization.matmul_nbits_quantizer import MatMulNBitsQuantizer

    dst = os.path.join(OUT, "model_int4.onnx")
    q = MatMulNBitsQuantizer(base, bits=4, block_size=128, is_symmetric=True)
    q.process()
    q.model.save_model_to_file(dst, use_external_data_format=False)
    print(f"model_int4.onnx: {os.path.getsize(dst)/1e6:.1f} MB")
    os.remove(base)
    return dst


# ---------- 3. 打包词嵌入表 ----------
def pack_nbit(q, bits):
    """q: uint8 array [rows, dim]，值域 [0, 2**bits-1]。

    位流按行主序：元素 e 的 bits 个比特依次（LSB 优先）排在位流 [e*bits, e*bits+bits)，
    再按 MSB-first（每字节高位在前）打包。与 Rust `EmbedTable::unpack` 一致。
    """
    vals = q.reshape(-1)
    stream = np.empty(vals.size * bits, dtype=np.uint8)
    for b in range(bits):
        stream[b::bits] = (vals >> b) & 1
    return np.packbits(stream).tobytes()


def pack_embedding(bits=2, groupsz=64):
    fp = np.fromfile(
        os.path.join(OUT, "word_emb_fp32.bin"), dtype=np.float32
    ).reshape(VOCAB, EMBED_DIM)
    ng = EMBED_DIM // groupsz
    fpg = fp.reshape(VOCAB, ng, groupsz)
    s = (fpg.max(axis=2, keepdims=True) - fpg.min(axis=2, keepdims=True)) / float(2**bits - 1)
    z = fpg.min(axis=2, keepdims=True)
    s = np.maximum(s, 1e-9)
    q = (
        np.clip(np.round((fpg - z) / s), 0, 2**bits - 1)
        .astype(np.uint8)
        .reshape(VOCAB, EMBED_DIM)
    )
    data = pack_nbit(q, bits)
    scales = np.concatenate([s, z], axis=2).reshape(VOCAB, ng * 2).astype(np.float16).tobytes()
    err = (q.astype(np.float32).reshape(VOCAB, ng, groupsz) * s + z) - fpg
    rmse = float(np.sqrt((err**2).mean()))
    rel = rmse / float(np.sqrt((fp**2).mean()))
    print(
        f"bits={bits} group={groupsz}: data {len(data)/1e6:.2f}MB "
        f"scales {len(scales)/1e6:.2f}MB rel={rel:.3f}"
    )
    scales_per_row = ng * 2
    header = f"BEKKOEMB{bits:02d}|{VOCAB}|{EMBED_DIM}|{scales_per_row}\n".encode()
    fname = os.path.join(OUT, "embedding_int2.bin")
    with open(fname, "wb") as f:
        f.write(header)
        f.write(scales)
        f.write(data)
    print(f"  -> {fname} {os.path.getsize(fname)/1e6:.2f}MB")
    os.remove(os.path.join(OUT, "word_emb_fp32.bin"))


# ---------- 4. tokenizer + 元信息 ----------
def tokenizer():
    with open(TOKENIZER_SRC, encoding="utf-8") as f:
        d = json.load(f)
    dst = os.path.join(OUT, "tokenizer.json")
    with open(dst, "w", encoding="utf-8") as f:
        json.dump(d, f, ensure_ascii=False, separators=(",", ":"))
    gz = dst + ".gz"
    with open(dst, "rb") as fin, gzip.GzipFile(gz, "wb", compresslevel=9, mtime=0) as fout:
        shutil.copyfileobj(fin, fout)
    os.remove(dst)
    print(f"tokenizer.json.gz: {os.path.getsize(gz)/1e6:.2f}MB")


def config():
    meta = {
        "model": "BAAI/bge-small-zh-v1.5",
        "embed_dim": EMBED_DIM,
        "vocab": VOCAB,
        "max_seq_len": 512,
        "pooling": "cls",
        "normalize": True,
        "embedding_quant": "int2+grouped-affine",
        "transformer_quant": "int4-matmulnbits",
    }
    with open(os.path.join(OUT, "config.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)
    print("config.json written")


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else "all"
    if what in ("all", "surgery"):
        surgery()
    if what in ("all", "quant"):
        quantize_int4(os.path.join(OUT, "base_fp32.onnx"))
    if what in ("all", "emb"):
        pack_embedding(2, 64)
    if what in ("all", "tok"):
        tokenizer()
    if what in ("all", "config"):
        config()
