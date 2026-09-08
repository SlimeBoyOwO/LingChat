#!/usr/bin/env python3
"""把 sentence-transformers 目录中的 transformer 骨干导出为 ONNX。

仅导出 transformer 主体（last_hidden_state），均值池化由 server.py 本地完成，
与 m3e 等 BERT 系模型的 `1_Pooling/config.json` (pooling_mode_mean_tokens) 一致。

用法：
    python export_onnx.py --model-dir <dir>

依赖（一次性）：torch + transformers；导出完成后可卸载，运行时只需 onnxruntime+numpy。
输出：<dir>/model.onnx（动态 batch/seq）。
"""
import argparse
import json
import os
import sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model-dir", required=True, help="sentence-transformers 模型目录")
    args = ap.parse_args()

    model_dir = os.path.abspath(args.model_dir)
    out_path = os.path.join(model_dir, "model.onnx")
    if os.path.exists(out_path):
        print(f"已存在 {out_path}，跳过导出")
        return 0

    import numpy as np
    import onnxruntime as ort
    import torch  # noqa: F401
    from transformers import AutoModel

    model = AutoModel.from_pretrained(model_dir, trust_remote_code=True)
    model.eval()

    hidden = model.config.hidden_size
    dynamic_axes = {
        "input_ids": {0: "B", 1: "S"},
        "attention_mask": {0: "B", 1: "S"},
        "token_type_ids": {0: "B", 1: "S"},
        "last_hidden_state": {0: "B", 1: "S"},
    }
    dummy = {
        "input_ids": torch.ones((1, 8), dtype=torch.long),
        "attention_mask": torch.ones((1, 8), dtype=torch.long),
        "token_type_ids": torch.zeros((1, 8), dtype=torch.long),
    }

    try:
        # torch 2.9 新 dynamo 导出器可能把 LayerNorm 的 batch 维烘焙为 1，
        # 优先使用 legacy tracer（对动态 batch/seq 的 BERT 图更稳妥）。
        torch.onnx.export(
            model,
            tuple(dummy.values()),
            out_path,
            input_names=list(dummy),
            output_names=["last_hidden_state"],
            dynamic_axes=dynamic_axes,
            opset_version=14,
            do_constant_folding=False,
            dynamo=False,
        )
    except (TypeError, ValueError):
        torch.onnx.export(
            model,
            tuple(dummy.values()),
            out_path,
            input_names=list(dummy),
            output_names=["last_hidden_state"],
            dynamic_axes=dynamic_axes,
            opset_version=14,
            do_constant_folding=False,
        )

    sess = ort.InferenceSession(out_path, providers=("CPUExecutionProvider",))
    out = sess.run(None, {
        "input_ids": np.ones((1, 8), dtype=np.int64),
        "attention_mask": np.ones((1, 8), dtype=np.int64),
        "token_type_ids": np.zeros((1, 8), dtype=np.int64),
    })[0]
    assert out.shape[-1] == hidden, f"维度不符: {out.shape[-1]} != {hidden}"
    print(f"导出成功: {out_path} dim={hidden} (动态 batch/seq)")
    return 0


if __name__ == "__main__":
    sys.exit(main())