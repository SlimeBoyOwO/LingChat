#!/usr/bin/env bash
# 准备 LingChat 记忆嵌入所需的模型文件。
#
# 本脚本完成以下步骤：
#   1. 创建临时 Python 虚拟环境（仅用于一次性模型下载和 ONNX 导出）
#   2. 下载 sentence-transformers 格式的多语言嵌入模型（默认
#      sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2）
#   3. 导出 ONNX 格式模型（需临时安装 torch + transformers）
#   4. 对 ONNX 模型做 int4 全量化（model_quantized.onnx，Transformer MatMul→4bit、
#      Embedding 表→uint4 分块，MiniLM 实测 450MB → 68MB）
#   5. 清理非运行时文件（pytorch_model.bin、README.md 等）
#   6. 删除临时虚拟环境
#
# 运行时只需 Rust 侧 ONNX Runtime 推理，不再依赖 Python。
#
# 用法:
#   bash scripts/embedding/setup.sh [model_id] [model_dir]
#   bash scripts/embedding/setup.sh --cleanup [model_dir]  # 仅清理已下载模型
#
# 环境变量:
#   PYTHON         指定 python 解释器（默认查找 python3/python）
#   VENV_DIR       虚拟环境目录（默认 <repo>/.venv-embedding）
#   NO_DOWNLOAD    设为 1 时跳过模型下载（便于离线/已有模型）

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODEL_ID="${1:-sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2}"
TARGET_DIR="${2:-$REPO_ROOT/data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2}"
DOWNLOAD="${NO_DOWNLOAD:-0}"

# 支持 --cleanup 模式：仅清理已下载模型中的非运行时文件
if [ "${1:-}" = "--cleanup" ]; then
  CLEANUP_DIR="${2:-$REPO_ROOT/data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2}"
  echo "── 清理模型目录: $CLEANUP_DIR"
  python3 "$REPO_ROOT/scripts/embedding/quantize_onnx.py" --model-dir "$CLEANUP_DIR" --cleanup 2>/dev/null || true
  # 手动清理（即使量化脚本不可用）
  for f in pytorch_model.bin README.md modules.json sentence_bert_config.json special_tokens_map.json .gitattributes; do
    rm -f "$CLEANUP_DIR/$f"
  done
  rm -rf "$CLEANUP_DIR/1_Pooling" "$CLEANUP_DIR/.cache"
  echo "完成。"
  exit 0
fi

VENV_DIR="${VENV_DIR:-$REPO_ROOT/.venv-embedding}"

PYTHON_BIN="${PYTHON:-}"
if [ -z "$PYTHON_BIN" ]; then
  if command -v python3 >/dev/null 2>&1; then PYTHON_BIN=python3
  elif command -v python >/dev/null 2>&1; then PYTHON_BIN=python
  else echo "未找到 Python，请安装 Python 3.9+ 并设置 PYTHON 环境变量" >&2; exit 1; fi
fi

echo "── 创建临时虚拟环境: $VENV_DIR"
"$PYTHON_BIN" -m venv "$VENV_DIR"
"$VENV_DIR/bin/python" -m pip install --upgrade pip >/dev/null
"$VENV_DIR/bin/python" -m pip install onnxruntime numpy tokenizers huggingface_hub

if [ "$DOWNLOAD" = "1" ]; then
  echo "── 跳过模型下载（NO_DOWNLOAD=1）"
else
  echo "── 下载嵌入模型: $MODEL_ID → $TARGET_DIR"
  # 中国大陆网络建议设置 HF_ENDPOINT=https://hf-mirror.com 走镜像
  "$VENV_DIR/bin/python" - <<PY
import os
os.makedirs(r"$TARGET_DIR", exist_ok=True)
from huggingface_hub import snapshot_download
endpoint = os.environ.get("HF_ENDPOINT")
if endpoint:
    os.environ["HF_ENDPOINT"] = endpoint
snapshot_download(repo_id="$MODEL_ID", local_dir=r"$TARGET_DIR")
print("模型下载完成")
PY
fi

echo "── 导出 ONNX（一次性，需临时 torch）"
# 多数 sentence-transformers 仓库不内置 ONNX；用 torch 导出一次后卸载，
# 运行时仅需 ONNX Runtime 推理，不再依赖 Python。
if "${VENV_DIR}/bin/python" -c "import os,sys; sys.exit(0 if os.path.exists(r'$TARGET_DIR/model.onnx') else 1)"; then
  : # 已有 model.onnx，跳过
else
  "$VENV_DIR/bin/python" -m pip install --index-url https://download.pytorch.org/whl/cpu torch 2>/dev/null \
    || "$VENV_DIR/bin/python" -m pip install torch
  "$VENV_DIR/bin/python" -m pip install transformers 2>/dev/null \
    || "$VENV_DIR/bin/python" -m pip install transformers
  "$VENV_DIR/bin/python" "$REPO_ROOT/scripts/embedding/export_onnx.py" --model-dir "$TARGET_DIR"
fi

echo "── int4 全量化 (MatMulNBits + uint4 嵌入表)"
"$VENV_DIR/bin/python" "$REPO_ROOT/scripts/embedding/quantize_onnx.py" --model-dir "$TARGET_DIR"

echo "── 清理非运行时文件"
# 删除 PyTorch 原始权重、HuggingFace 元数据、下载缓存、源 ONNX（量化产物已内联权重）
for f in pytorch_model.bin README.md modules.json sentence_bert_config.json special_tokens_map.json .gitattributes model.onnx model.onnx.data; do
  rm -f "$TARGET_DIR/$f"
done
rm -rf "$TARGET_DIR/1_Pooling" "$TARGET_DIR/.cache"

echo "── 删除临时虚拟环境"
rm -rf "$VENV_DIR"

echo ""
echo "完成。嵌入模型目录: $TARGET_DIR"
echo "运行时只需: model_quantized.onnx + tokenizer.json + tokenizer_config.json + config.json"
