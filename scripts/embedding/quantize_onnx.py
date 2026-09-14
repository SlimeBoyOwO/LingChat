#!/usr/bin/env python3
"""对 ONNX 嵌入模型做 int4 全量化，显著减小模型体积。

方案（m3e-small 实测 93MB → 15.3MB，比 int8 再省 ~36%）：
  1. Transformer 的 MatMul/Gemm 权重 → MatMulNBits 4-bit（onnxruntime 官方算子）
  2. Embedding 表（word/position/token_type）→ uint4 分块量化 +
     DequantizeLinear(block_size) + Cast 还原，运行时质量几乎无损

用法：
    python quantize_onnx.py --model-dir <dir>

输入：<dir>/model.onnx + model.onnx.data
输出：<dir>/model_quantized.onnx（权重内联，无 .data 文件）

依赖：onnxruntime >=1.20 + onnx + numpy
"""
import argparse
import os
import sys

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

BLOCK = 32  # uint4 嵌入表分块大小；越小精度越高、scale 开销越大


def _make_uint4(name: str, dims: list, raw: bytes) -> TensorProto:
    t = TensorProto()
    t.name = name
    t.data_type = TensorProto.UINT4
    t.dims.extend(dims)
    t.raw_data = raw
    return t


def quantize_embeddings_uint4(model: onnx.ModelProto, block: int) -> int:
    """把被 Gather 消费的 float32 嵌入表量化为 uint4 分块格式。

    返回量化成功的张量个数。模型结构不匹配（无 Gather/嵌入表）时跳过。
    """
    consumed = {n.input[0] for n in model.graph.node if n.op_type == "Gather"}
    quantized = 0

    for init in list(model.graph.initializer):
        if init.name not in consumed or init.data_type != TensorProto.FLOAT:
            continue
        arr = numpy_helper.to_array(init)
        if arr.ndim != 2 or arr.shape[1] % (block * 2) != 0:
            continue

        n, k = arr.shape
        blocks = arr.reshape(n, k // block, block)
        bmin, bmax = blocks.min(axis=2), blocks.max(axis=2)
        scale = np.maximum((bmax - bmin) / 15.0, 1e-10).astype(np.float16)
        zp_f = np.clip(np.round(-bmin / scale), 0, 15).astype(np.uint8)
        q = np.clip(
            np.round(blocks / scale[:, :, None].astype(np.float32)
                     + zp_f[:, :, None].astype(np.float32)),
            0, 15,
        ).astype(np.uint8)
        q_flat = q.reshape(n, k)
        packed = (q_flat[:, 0::2] & 0x0F) | ((q_flat[:, 1::2] & 0x0F) << 4)

        zp_flat = zp_f.reshape(n, k // block)
        zp_packed = (zp_flat[:, 0::2] & 0x0F) | ((zp_flat[:, 1::2] & 0x0F) << 4)

        q_init = _make_uint4(init.name + "_q", [n, k], packed.tobytes())
        sc_init = numpy_helper.from_array(
            scale.reshape(n, k // block), name=init.name + "_scale")
        zp_init = _make_uint4(
            init.name + "_zp", [n, k // block], zp_packed.tobytes())

        model.graph.initializer.remove(init)
        model.graph.initializer.extend([q_init, sc_init, zp_init])

        dq_name = init.name + ".dq"
        dq_out = init.name + ".dq_f32"
        dq = helper.make_node(
            "DequantizeLinear",
            inputs=[q_init.name, sc_init.name, zp_init.name],
            outputs=[dq_name], block_size=block, axis=1,
        )
        cast = helper.make_node(
            "Cast", inputs=[dq_name], outputs=[dq_out], to=TensorProto.FLOAT)

        # 所有消费 Gather 改读 DQ 输出；DQ→Cast 只在首个 Gather 前插入一次
        first_gather = None
        for node in model.graph.node:
            if node.op_type == "Gather" and node.input[0] == init.name:
                node.input[0] = dq_out
                if first_gather is None:
                    first_gather = node
        assert first_gather is not None, f"嵌入表 {init.name} 无 Gather 消费者"
        new_nodes = []
        for node in model.graph.node:
            if node is first_gather:
                new_nodes.extend([dq, cast, node])
            else:
                new_nodes.append(node)
        del model.graph.node[:]
        model.graph.node.extend(new_nodes)
        quantized += 1

    return quantized


def quantize_model(model_dir: str) -> None:
    from onnxruntime.quantization.matmul_nbits_quantizer import MatMulNBitsQuantizer

    onnx_path = os.path.join(model_dir, "model.onnx")
    out_path = os.path.join(model_dir, "model_quantized.onnx")

    if not os.path.exists(onnx_path):
        print(f"错误：找不到 {onnx_path}", file=sys.stderr)
        sys.exit(1)

    orig_size = os.path.getsize(onnx_path)
    data_path = onnx_path + ".data"
    if os.path.exists(data_path):
        orig_size += os.path.getsize(data_path)

    print(f"量化中: {onnx_path}")
    print(f"原始大小: {orig_size / 1024 / 1024:.1f} MB")

    # 1) Transformer MatMul → MatMulNBits 4-bit
    model = onnx.load(onnx_path, load_external_data=True)
    q = MatMulNBitsQuantizer(model, bits=4, block_size=BLOCK, is_symmetric=False)
    q.process()
    model = q.model.model

    # 2) Embedding 表 → uint4 分块量化
    n_emb = quantize_embeddings_uint4(model, BLOCK)
    print(f"嵌入表 int4 量化: {n_emb} 张")

    # 3) uint4 块量化 DequantizeLinear 需 opset >= 21
    for oi in model.opset_import:
        if oi.domain in ("", "ai.onnx"):
            oi.version = max(oi.version, 21)
    if model.ir_version < 10:
        model.ir_version = 10

    onnx.save(model, out_path, save_as_external_data=False)

    quant_size = os.path.getsize(out_path)
    ratio = (1.0 - quant_size / orig_size) * 100 if orig_size > 0 else 0
    print(f"完成: {out_path}")
    print(f"量化后大小: {quant_size / 1024 / 1024:.1f} MB (缩减 {ratio:.0f}%)")

    # 4) 冒烟验证：能加载并推理即通过
    import onnxruntime as ort
    sess = ort.InferenceSession(out_path, providers=["CPUExecutionProvider"])
    inputs = sess.get_inputs()
    feeds = {}
    for i in inputs:
        shape = [1, 4]
        dt = {"int32": np.int32, "int64": np.int64}.get(i.type.split("(")[1].rstrip(")"), np.float32)
        feeds[i.name] = np.array([[101, 200, 201, 102]], dtype=dt)
    assert "attention_mask" in feeds or len(inputs) == 1
    if "attention_mask" in [i.name for i in inputs]:
        feeds["attention_mask"] = np.ones((1, 4), dtype=feeds["input_ids"].dtype)
    if "token_type_ids" in [i.name for i in inputs]:
        feeds["token_type_ids"] = np.zeros((1, 4), dtype=feeds["input_ids"].dtype)
    sess.run(None, feeds)
    print("冒烟验证通过：模型可正常加载推理")


def cleanup_model_dir(model_dir: str, keep_quantized: bool = True) -> None:
    """删除模型目录中运行时不需要的文件。"""
    remove_files = [
        "pytorch_model.bin",
        "README.md",
        "modules.json",
        "sentence_bert_config.json",
        "special_tokens_map.json",
        ".gitattributes",
    ]
    remove_dirs = [
        "1_Pooling",
        ".cache",
    ]
    # 量化产物已内联全部权重，源 ONNX 不再需要
    if keep_quantized and os.path.exists(os.path.join(model_dir, "model_quantized.onnx")):
        remove_files += ["model.onnx", "model.onnx.data"]

    removed = []
    for name in remove_files:
        p = os.path.join(model_dir, name)
        if os.path.exists(p):
            os.remove(p)
            removed.append(name)

    for name in remove_dirs:
        p = os.path.join(model_dir, name)
        if os.path.isdir(p):
            import shutil
            shutil.rmtree(p)
            removed.append(name + "/")

    if removed:
        print(f"已清理: {', '.join(removed)}")
    else:
        print("无需清理的文件")


def main() -> int:
    ap = argparse.ArgumentParser(description="ONNX 嵌入模型 int4 全量化")
    ap.add_argument("--model-dir", required=True, help="模型目录（含 model.onnx）")
    ap.add_argument("--cleanup", action="store_true", help="同时清理非运行时文件")
    ap.add_argument("--block", type=int, default=BLOCK, help=f"uint4 分块大小（默认 {BLOCK}）")
    args = ap.parse_args()

    model_dir = os.path.abspath(args.model_dir)
    if not os.path.isdir(model_dir):
        print(f"错误：目录不存在 {model_dir}", file=sys.stderr)
        return 1

    quantize_model(model_dir)

    if args.cleanup:
        cleanup_model_dir(model_dir)

    return 0


if __name__ == "__main__":
    sys.exit(main())