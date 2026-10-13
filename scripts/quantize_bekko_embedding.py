#!/usr/bin/env python3
"""量化 bekko-embedding-v1-a25m → 内置本地嵌入模型产物。

产物会随仓库分发（data/third_party/bekko_embedding_v1_a25m/，合计约 49MB）：

    model_int4.onnx      13.5MB  Transformer 全部 MatMul int4 weight-only（MatMulNBits）
    embedding_int2.bin   30.7MB  256000×384 词嵌入表 int2 + 每行 6 组 × fp16(scale,zero)
    tokenizer.json.gz    4.8MB   BPE 词表 gzip
    config.json          参考元信息（384 维 / 均值池化 / 8192 上下文）

流程：
1. 图手术：把 int8 词嵌入表（98.3MB 初始化器）换成外部 fp32 输入 `inputs_embeds`，
   接到原来的 LayerNormalization，其余图不动；
2. Transformer MatMul 全部 int4 weight-only 量化；
3. 词嵌入表反量化成 fp32 重建后，再按 2-bit + 分组 scale/zero 重打包（`pack_nbit`）；
4. tokenizer.json 紧凑化 + gzip（gzip -9，mtime 固定可复现）。

用法（需 Python ≥3.10 + onnx + onnxruntime + numpy）：
    uv venv /tmp/bekko-venv --python 3.12
    uv pip install --python /tmp/bekko-venv/bin/python \
        onnx onnxruntime numpy tokenizers onnx_ir
    # 原始 unquantized ONNX（HF 仓库 hotchpotch/bekko-embedding-v1-a25m onnx/engine
    # 分支，hf-mirror.com 加速下载）：
    curl -sSL -o bekko-model.onnx \\
        https://hf-mirror.com/hotchpotch/bekko-embedding-v1-a25m/resolve/main/onnx/model.onnx
    curl -sSL -o bekko-tokenizer.json \\
        https://hf-mirror.com/hotchpotch/bekko-embedding-v1-a25m/resolve/main/tokenizer.json
    python build_bekko.py
"""
import gzip
import json
import os
import shutil
import sys

import numpy as np
import onnx
from onnx import TensorProto, helper

# 原始模型路径与输出目录可用环境变量覆盖。
SRC = os.environ.get("BEKKO_SRC", "/tmp/opencode/bekko-model.onnx")
TOKENIZER_SRC = os.environ.get("BEKKO_TOKENIZER", "/tmp/opencode/bekko-tokenizer.json")
OUT = os.environ.get(
    "BEKKO_OUT",
    os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                 "data", "third_party", "bekko_embedding_v1_a25m"),
)
os.makedirs(OUT, exist_ok=True)

EMBED_IN = "inputs_embeds"
EMBED_DIM = 384
VOCAB = 256000


# ---------- 1. 图手术 ----------
def surgery():
    m = onnx.load(SRC, load_external_data=False)
    g = m.graph

    # 定位反量化链：Gather → GatherRowwiseScale → Cast → RestoreRowwiseInt8
    dequant_names = {
        "/embeddings/tok_embeddings/Gather",
        "/static_small_embeddings/GatherRowwiseScale",
        "/static_small_embeddings/CastQuantizedEmbeddingToFloat",
        "/static_small_embeddings/RestoreRowwiseInt8",
    }
    dequant_nodes = [n for n in g.node if n.name in dequant_names]
    assert len(dequant_nodes) == 4, [n.name for n in dequant_nodes]
    dequant_out = "/embeddings/tok_embeddings/Gather_output_0"

    # 确认唯一消费者是 LayerNormalization
    consumers = [n for n in g.node if dequant_out in n.input]
    assert len(consumers) == 1 and consumers[0].op_type == "LayerNormalization", [
        (n.op_type, n.name) for n in consumers
    ]

    # 删反量化链节点
    kept = [n for n in g.node if n.name not in dequant_names]
    del g.node[:]
    g.node.extend(kept)

    # 新增 fp32 输入，接到 LayerNormalization
    emb_in = helper.make_tensor_value_info(
        EMBED_IN, TensorProto.FLOAT, ["batch_size", "sequence_length", EMBED_DIM]
    )
    g.input.append(emb_in)
    for n in g.node:
        if dequant_out in n.input:
            idx = list(n.input).index(dequant_out)
            n.input[idx] = EMBED_IN

    # 删掉不再使用的初始化器（int8 表 + rowwise scale）
    used = set()
    for n in g.node:
        used.update(n.input)
    keep_init = [t for t in g.initializer if t.name in used]
    removed = [t.name for t in g.initializer if t.name not in used]
    print(f"removed initializers: {removed}")
    for t in g.initializer:
        if t.name not in used:
            raw = t.raw_data or b""
            print(f"  {t.name}: data_type={t.data_type} dims={list(t.dims)} raw={len(raw)/1e6:.1f}MB")
    del g.initializer[:]
    g.initializer.extend(keep_init)

    onnx.checker.check_model(m, full_check=False)
    base = os.path.join(OUT, "base_fp32.onnx")
    onnx.save(m, base)
    print(f"base_fp32.onnx: {os.path.getsize(base)/1e6:.1f} MB")
    return base


# ---------- 2. int4 量化 ----------
def quantize_int4(base):
    from onnxruntime.quantization import QuantType, quantize_dynamic

    dst = os.path.join(OUT, "model_int4.onnx")
    quantize_dynamic(
        model_input=base,
        model_output=dst,
        per_channel=False,
        reduce_range=False,
        weight_type=QuantType.QInt4,
        op_types_to_quantize=["MatMul"],
        extra_options={
            "MatMulConstBOnly": True,
            "WeightSymmetric": True,
            "ActivationSymmetric": True,
        },
    )
    print(f"model_int4.onnx: {os.path.getsize(dst)/1e6:.1f} MB")
    return dst


# ---------- 3. 打包词嵌入表 ----------
def pack_embedding(bits, groupsz=None):
    m = onnx.load(SRC, load_external_data=False)
    g = m.graph
    init = {t.name: t for t in g.initializer}
    scale_init_name = "static_small_embeddings.rowwise_scale"
    rg = next(n for n in g.node if n.name == "/static_small_embeddings/GatherRowwiseScale")
    assert scale_init_name in init and scale_init_name in rg.input, (
        list(rg.input),
        sorted(init)[:5],
    )
    w8 = onnx.numpy_helper.to_array(init["embeddings.tok_embeddings.weight"])
    scale = onnx.numpy_helper.to_array(init[scale_init_name]).astype(np.float32)
    print("int8 table:", w8.shape, "scale:", scale.shape, scale[:3])

    # 反量化成 fp32（重建原始近似）；scale 形状 [256000,1]
    fp = w8.astype(np.float32) * scale  # [256000, 384]
    assert fp.shape == (VOCAB, EMBED_DIM)

    if bits == 8:
        # 直接存 int8 + 每行 f16 scale（对照）
        mx = np.max(np.abs(fp), axis=1, keepdims=True)
        s = np.maximum(mx / 127.0, 1e-9)
        q = np.clip(np.round(fp / s), -127, 127).astype(np.int8)
        data = q.tobytes()
        scales = s.astype(np.float16).tobytes()
        err = np.abs(q * s - fp)
    elif bits in (2, 3):
        gs = groupsz or EMBED_DIM
        ng = EMBED_DIM // gs
        assert EMBED_DIM % gs == 0
        fpg = fp.reshape(VOCAB, ng, gs)
        s = (fpg.max(axis=2, keepdims=True) - fpg.min(axis=2, keepdims=True)) / float(2**bits - 1)
        z = fpg.min(axis=2, keepdims=True)
        s = np.maximum(s, 1e-9)
        q = np.clip(np.round((fpg - z) / s), 0, 2**bits - 1).astype(np.uint8)
        q = q.reshape(VOCAB, EMBED_DIM)
        data = pack_nbit(q, bits)
        scales = (
            np.concatenate([s, z], axis=2).reshape(VOCAB, ng * 2).astype(np.float16).tobytes()
        )
        err = (q.astype(np.float32).reshape(VOCAB, ng, gs) * s + z) - fpg
    else:
        raise ValueError(bits)

    rmse = float(np.sqrt((err**2).mean()))
    rel = rmse / float(np.sqrt((fp**2).mean()))
    print(
        f"bits={bits} group={groupsz or EMBED_DIM}: data {len(data)/1e6:.2f}MB "
        f"scales {len(scales)/1e6:.2f}MB rmse={rmse:.4f} rel={rel:.3f}"
    )

    gs = groupsz or EMBED_DIM
    fname = os.path.join(OUT, f"embedding_b{bits}_g{gs}.bin")
    scales_per_row = (EMBED_DIM // gs) * (1 if bits == 8 else 2)
    header = f"BEKKOEMB{bits:02d}|{VOCAB}|{EMBED_DIM}|{scales_per_row}\n".encode()
    with open(fname, "wb") as f:
        f.write(header)
        f.write(scales)
        f.write(data)
    print(f"  -> {fname} {os.path.getsize(fname)/1e6:.2f}MB")
    return fname


def pack_nbit(q, bits):
    """q: uint8 array [rows, 384]，值域 [0, 2**bits-1]。

    位流按行主序：元素 e 的 bits 个比特依次（LSB 优先）排在
    位流 [e*bits, e*bits+bits)，再按 MSB-first 打包成字节。
    与 Rust 端 `EmbedTable::unpack` 保持一致。
    """
    vals = q.reshape(-1)
    stream = np.empty(vals.size * bits, dtype=np.uint8)
    for b in range(bits):
        stream[b::bits] = (vals >> b) & 1
    return np.packbits(stream).tobytes()


# ---------- 4. tokenizer ----------
def tokenizer():
    d = json.load(open(TOKENIZER_SRC))
    dst = os.path.join(OUT, "tokenizer.json")
    with open(dst, "w", encoding="utf-8") as f:
        json.dump(d, f, ensure_ascii=False, separators=(",", ":"))
    sz = os.path.getsize(dst)
    gz = dst + ".gz"
    with open(dst, "rb") as fin, gzip.GzipFile(gz, "wb", compresslevel=9, mtime=0) as fout:
        shutil.copyfileobj(fin, fout)
    print(f"tokenizer.json {sz/1e6:.1f}MB -> gz {os.path.getsize(gz)/1e6:.1f}MB")


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else "all"
    if what in ("all", "surgery"):
        base = surgery()
    if what in ("all", "quant"):
        base = os.path.join(OUT, "base_fp32.onnx")
        quantize_int4(base)
    if what in ("all", "emb"):
        pack_embedding(2)             # 1 组/行（基准）
        pack_embedding(2, groupsz=96)  # 4 组/行（更准）
        pack_embedding(2, groupsz=64)  # 6 组/行（内置产物采用，文件需重命名成 embedding_int2.bin）
    if what in ("all", "tok"):
        tokenizer()