#!/usr/bin/env python3
"""嵌入模型子进程服务（stdio JSON 协议）。

为 LingChat 记忆系统提供中英双语文本嵌入向量，供语义检索与去重使用。

协议（stdin/stdout 各一行一个 JSON 对象，UTF-8）：
    请求: {"action": "ping"}
    响应: {"ok": true, "ready": true, "dim": 512, "model": "..."}

    请求: {"action": "encode", "texts": ["...", ...]}        # 小批量
    响应: {"ok": true, "vectors": [[0.1, ...], ...]}

    请求: {"action": "similarity", "a": ["..."], "b": ["..."]}
    响应: {"ok": true, "similarities": [[0.9, ...], ...]}

    请求: {"action": "shutdown"}
    响应: {"ok": true}

约定：
    - 模型目录通过命令行参数或环境变量 EMBEDDING_MODEL_DIR 传入。
    - 优先使用 onnxruntime 直跑（小、快、无需 torch）；若模型是标准的
      sentence-transformers 目录结构则直接可用。
    - 每条请求读取一行，处理完写一行，支持长驻进程，避免反复加载模型。

后端选型：
    - 若检测到模型目录里有 model.onnx / model_quantized.onnx 且提供
      tokenizer.json（或 vocab.txt），走纯 ONNX 路径（推荐，体积最小）。
    - 否则尝试用 sentence-transformers（需要 torch + transformers + 网络/缓存）。
"""

import json
import os
import sys
import argparse

# 默认嵌入模型（sentence-transformers 格式，多语言）。用户可改 EMBEDDING_MODEL_DIR。
# 注意：当前 Rust 侧为原生 ONNX 推理；此脚本为旧版 stdio 服务，已被 export_onnx.py
# + quantize_onnx.py + Rust 直推理取代。
DEFAULT_MODEL = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"

# 允许传入的单个文本最长字符（防滥用）
MAX_TEXT_CHARS = 20000
# 单次 encode 批量上限
MAX_BATCH = 128


class EmbeddingServer:
    """加载模型并通过 stdio 提供服务。"""

    def __init__(self, model_dir, backend="auto", max_length=512):
        self.model_dir = model_dir
        self.backend = backend
        self.max_length = max_length
        self._dim = None
        self._model = None
        self._load()

    # ---- 加载 ----

    def _load(self):
        if self.backend in ("auto", "onnx"):
            try:
                self._load_onnx()
                return
            except Exception as e:  # noqa: BLE001
                if self.backend == "onnx":
                    raise
                sys.stderr.write(f"[embedding] ONNX 加载失败，回退 sentence-transformers: {e}\n")
        if self.backend in ("auto", "st"):
            self._load_sentence_transformers()
            return
        raise RuntimeError(f"未知后端: {self.backend}")

    def _find_onnx_model(self):
        if not self.model_dir:
            return None
        for name in ("model_quantized.onnx", "model_int8.onnx", "model.onnx", "onnx/model.onnx"):
            p = os.path.join(self.model_dir, name)
            if os.path.exists(p):
                return p
        return None

    def _find_tokenizer(self, onnx_path):
        # 优先 tokenizer.json（wordpiece/BPE/Unigram 均可）
        candidates = [
            os.path.join(self.model_dir, "tokenizer.json"),
            os.path.join(os.path.dirname(onnx_path), "tokenizer.json"),
            os.path.join(self.model_dir, "vocab.txt"),
        ]
        cfg = None
        cfg_path = os.path.join(self.model_dir, "tokenizer_config.json")
        if os.path.exists(cfg_path):
            try:
                with open(cfg_path, "r", encoding="utf-8") as f:
                    cfg = json.load(f)
            except Exception:  # noqa: BLE001
                cfg = None
        for c in candidates:
            if c and os.path.exists(c):
                return c, cfg
        return None, None

    def _load_onnx(self):
        import numpy  # noqa: F401  确保 numpy 已装
        import onnxruntime as ort

        onnx_path = self._find_onnx_model()
        if not onnx_path:
            raise FileNotFoundError(f"模型目录缺少 ONNX 文件: {self.model_dir}")
        tokenizer_path, tok_cfg = self._find_tokenizer(onnx_path)
        if not tokenizer_path:
            raise FileNotFoundError(f"模型目录缺少 tokenizer.json/vocab.txt: {self.model_dir}")

        sess = ort.InferenceSession(onnx_path, providers=("CPUExecutionProvider",))
        inputs = [i.name for i in sess.get_inputs()]
        outputs = [o.name for o in sess.get_outputs()]

        self._onnx = (sess, inputs, outputs)
        self._tokenizer = _Tokenizer(tokenizer_path, tok_cfg, self.max_length)
        # 探测维度：用一行 dummy 输入跑一次
        dummy = self._tokenizer.encode(["test"])
        dummy_in = self._prep_onnx_inputs(dummy)
        out = sess.run(None, dummy_in)
        vec = out[0]
        # 归一化（sentence-transformers 约定）
        self._dim = int(vec.shape[-1])

        # 是否需要均值池化：输出 shape 为 [batch, seq, dim] → pool
        self._need_pool = len(vec.shape) == 3
        self._dim_out = self._dim if not self._need_pool else int(vec.shape[-1])
        self._dim = self._dim_out
        self._device = "cpu"
        self._backend_name = "onnx"
        # 归一化桶（用于均值池化后归一化）
        self._normalize = True
        sys.stderr.write(
            f"[embedding] ONNX 就绪: {onnx_path} dim={self._dim} inputs={inputs} outputs={outputs}\n"
        )

    def _load_sentence_transformers(self):
        from sentence_transformers import SentenceTransformer  # type: ignore

        model_path = self.model_dir or DEFAULT_MODEL
        self._st = SentenceTransformer(model_path, device="cpu")
        self._dim = int(self._st.get_sentence_embedding_dimension())
        self._backend_name = "sentence-transformers"
        sys.stderr.write(f"[embedding] sentence-transformers 就绪: {model_path} dim={self._dim}\n")

    # ---- ONNX 输入构造 ----

    def _prep_onnx_inputs(self, tok):
        input_ids, attention_mask, token_type_ids = tok
        sess, inputs, _ = self._onnx
        feed = {}
        if "input_ids" in inputs or "input_ids" in [i.lower() for i in inputs]:
            feed[inputs[0]] = input_ids
        else:
            feed[inputs[0]] = input_ids
        added = {inputs[0]}
        if "attention_mask" in inputs:
            feed["attention_mask"] = attention_mask
            added.add("attention_mask")
        elif any("mask" in i.lower() for i in inputs):
            m = [i for i in inputs if "mask" in i.lower()][0]
            feed[m] = attention_mask
            added.add(m)
        if "token_type_ids" in inputs and token_type_ids is not None:
            feed["token_type_ids"] = token_type_ids
            added.add("token_type_ids")
        # 其余输入（如 position_ids）给假零
        for i in inputs:
            if i not in added:
                feed[i] = input_ids
        return feed

    # ---- embed ----

    def encode(self, texts):
        cleaned = [_sanitize(t) for t in texts]
        if self._backend_name == "onnx":
            vecs = self._encode_onnx(cleaned)
        else:
            vecs = [v.tolist() for v in self._st.encode(cleaned, normalize_embeddings=True)]
        return vecs

    def _encode_onnx(self, texts):
        try:
            return self._run_onnx(texts)
        except Exception as e:  # noqa: BLE001
            # 某些 onnxruntime 版本（如 1.29）对 dynamic batch + LayerNorm 的
            # 模型会直接崩溃（如 m3e-small）。批量推理失败时退化为逐条编码，
            # 保证服务可用（单条推理已被探测阶段验证过）。
            sys.stderr.write(f"[embedding] 批量 ONNX 推理失败，退化为逐条编码: {e}\n")
            return [self._run_onnx([t])[0] for t in texts]

    def _run_onnx(self, texts):
        import numpy as np

        sess, _, _ = self._onnx
        tok = self._tokenizer.encode(texts)
        feed = self._prep_onnx_inputs(tok)
        out = sess.run(None, feed)[0]
        if self._need_pool:
            # 均值池化（attention mask 加权）
            input_ids, attention_mask, _ = tok
            mask = np.array(attention_mask, dtype=np.float32)
            mask_exp = np.expand_dims(mask, axis=-1)
            summed = np.sum(out * mask_exp, axis=1)
            counts = np.maximum(np.sum(mask, axis=1, keepdims=True), 1e-9)
            pooled = summed / counts
        else:
            pooled = out
        pooled = pooled.astype(np.float32)
        # L2 归一化
        norms = np.linalg.norm(pooled, axis=-1, keepdims=True)
        pooled = pooled / np.maximum(norms, 1e-12)
        return [r.tolist() for r in pooled]

    def similarity(self, a, b):
        va = self.encode(a)
        vb = self.encode(b)
        import math

        out = []
        for x in va:
            row = []
            for y in vb:
                row.append(_dot(x, y))
            out.append(row)
        return out

    def dim(self):
        return self._dim

    def model_name(self):
        if self._backend_name == "onnx":
            return f"onnx:{os.path.basename(self.model_dir) or 'embedding'}"
        return str(self.model_dir or DEFAULT_MODEL)


def _sanitize(text):
    if not isinstance(text, str):
        return ""
    if len(text) > MAX_TEXT_CHARS:
        return text[:MAX_TEXT_CHARS]
    return text


def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))


class _Tokenizer:
    """极简 ONNX 分词：支持 wordpiece(BERT系) 或 sentencepiece/unigram(tokenizer.json)。

    仅覆盖常见中文小模型：
      - m3e-small / text2vec 等 BERT 系：vocab.txt（wordpiece）+ 首尾 [CLS]/[SEP]
      - 带 tokenizer.json 的模型：尝试用词表直接查块；若拿不到则按 UTF-8 字节退化。
    """

    def __init__(self, path, config=None, max_length=512):
        self.max_length = max_length
        # 优先用 tokenizers 库做精确 WordPiece/BPE（与 transformers 一致）。
        # 未安装时退化为词表逐字符查（中文基本够用，英文子词稍有偏差）。
        self._fast = None
        if path.endswith(".json"):
            try:
                from tokenizers import Tokenizer as _FT
                from tokenizers import Encoding as _FE  # noqa: F401

                self._fast = _FT.from_file(path)
            except Exception:  # noqa: BLE001
                self._fast = None
        self.cls_id = 101 if config and config.get("tokenizer_class") != "BertTokenizer" else 101
        self.sep_id = 102
        self.pad_id = 0
        self.unk_id = 100
        # 读取 vocab / tokenizer.json
        self.word_lookup = None
        if path.endswith(".json"):
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            vocab = data.get("model", {}).get("vocab") if isinstance(data.get("model"), dict) else None
            if isinstance(vocab, dict):
                self.word_lookup = {k: int(v) for k, v in vocab.items()}
                if config:
                    self.cls_id = config.get("cls_token_id", self.cls_id)
                    self.sep_id = config.get("sep_token_id", self.sep_id)
                    self.pad_id = config.get("pad_token_id", self.pad_id)
                    self.unk_id = config.get("unk_token_id", self.unk_id)
        else:
            with open(path, "r", encoding="utf-8") as f:
                self.word_lookup = {line.strip(): i for i, line in enumerate(f) if line.strip()}

    def encode(self, texts):
        import numpy as np

        if self._fast is not None:
            enc = self._fast.encode_batch(texts)
            max_len = max((len(e.ids) for e in enc), default=1)
            max_len = min(max_len, self.max_length)
            if max_len < 3:
                max_len = 3
            input_ids, attention_mask, token_type_ids = [], [], []
            for e in enc:
                ids = e.ids[:max_len]
                pad = max_len - len(ids)
                seq = ids + [self.pad_id] * max(0, pad)
                input_ids.append(seq[:max_len])
                attention_mask.append([1] * (max_len - max(0, pad)) + [0] * max(0, pad))
                token_type_ids.append([0] * max_len)
            return (
                np.array(input_ids, dtype=np.int64),
                np.array(attention_mask, dtype=np.int64),
                np.array(token_type_ids, dtype=np.int64),
            )

        batch = []
        for text in texts:
            ids = self._text_to_ids(text)
            batch.append(ids)
        max_len = max((len(x) for x in batch), default=1)
        max_len = min(max_len, self.max_length)
        if max_len < 3:
            max_len = 3
        input_ids, attention_mask, token_type_ids = [], [], []
        for ids in batch:
            ids = ids[: max_len - 1]
            seq = [self.cls_id] + ids + [self.sep_id]
            pad = max_len - len(seq)
            seq = seq + [self.pad_id] * max(0, pad)
            mask = [1] * (max_len - max(0, pad)) + [0] * max(0, pad)
            ttype = [0] * max_len
            input_ids.append(seq[:max_len])
            attention_mask.append(mask[:max_len])
            token_type_ids.append(ttype)
        return (
            np.array(input_ids, dtype=np.int64),
            np.array(attention_mask, dtype=np.int64),
            np.array(token_type_ids, dtype=np.int64),
        )

    def _text_to_ids(self, text):
        ids = []
        # 整词优先
        if self.word_lookup:
            for char in text:
                if char in self.word_lookup:
                    ids.append(self.word_lookup[char])
                else:
                    # 尝试小写单词形式
                    low = char.lower()
                    ids.append(self.word_lookup.get(low, self.word_lookup.get("[UNK]", self.unk_id)))
        else:
            ids = [ord(c) for c in text]
        return ids


def _read_request():
    line = sys.stdin.readline()
    if not line:
        return None
    line = line.strip()
    if not line:
        return _read_request()
    return json.loads(line)


def _write_response(obj):
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main():
    ap = argparse.ArgumentParser(description="LingChat embedding subprocess server")
    ap.add_argument("--model-dir", default=None, help="嵌入模型目录（sentence-transformers 或 ONNX 格式）")
    ap.add_argument("--backend", default="auto", choices=["auto", "onnx", "st"])
    ap.add_argument("--max-length", type=int, default=512)
    args = ap.parse_args()

    model_dir = args.model_dir or os.environ.get("EMBEDDING_MODEL_DIR")
    try:
        server = EmbeddingServer(model_dir, backend=args.backend, max_length=args.max_length)
    except Exception as e:  # noqa: BLE001
        sys.stderr.write(f"[embedding] 模型加载失败，嵌入服务不可用: {e}\n")
        # 以失败状态退出，Rust 侧读到 EOF 后按不可用处理并记录诊断。
        sys.exit(2)

    _write_response({"ok": True, "ready": True, "dim": server.dim(), "model": server.model_name()})

    while True:
        try:
            req = _read_request()
        except Exception as e:  # noqa: BLE001
            _write_response({"ok": False, "error": f"bad request: {e}"})
            continue
        if req is None:
            break
        action = req.get("action")
        try:
            if action == "ping":
                _write_response({"ok": True, "ready": True, "dim": server.dim(), "model": server.model_name()})
            elif action == "encode":
                texts = req.get("texts", [])
                if not isinstance(texts, list) or len(texts) > MAX_BATCH:
                    _write_response({"ok": False, "error": "texts 必须是非空字符串数组（上限 128）"})
                    continue
                vecs = server.encode(texts)
                _write_response({"ok": True, "vectors": vecs, "dim": server.dim()})
            elif action == "similarity":
                a = req.get("a", [])
                b = req.get("b", [])
                sims = server.similarity(a, b)
                _write_response({"ok": True, "similarities": sims})
            elif action == "shutdown":
                _write_response({"ok": True})
                break
            else:
                _write_response({"ok": False, "error": f"unknown action: {action}"})
        except Exception as e:  # noqa: BLE001
            sys.stderr.write(f"[embedding] handle error: {e}\n")
            _write_response({"ok": False, "error": str(e)})


if __name__ == "__main__":
    main()
