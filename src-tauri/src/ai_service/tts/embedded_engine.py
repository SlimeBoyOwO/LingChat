# -*- coding: utf-8 -*-
"""
LingChat 进程内 IndexTTS2 最小推理桥。

生命周期、排队、音色索引、情绪决策/缓存、流式文件写入与 WAV 封装均由 Rust
负责；本文件只保留无法脱离 PyTorch/ROCm 的模型加载、Qwen 情绪向量推理和
逐段张量推理。它不导入 FastAPI、Uvicorn 或 server_indextts.py。
"""

import os
import sys

_original_stdout = sys.stdout
_original_stderr = sys.stderr

# 隔离模式嵌入 CPython 时必须在导入 torch 前设置绝对前缀。
_runtime_value = os.environ.get("INDEXTTS_RUNTIME_DIR", "").strip()
_runtime_root = os.path.abspath(_runtime_value) if _runtime_value else ""
if _runtime_root and os.path.isdir(_runtime_root):
    sys.prefix = _runtime_root
    sys.exec_prefix = _runtime_root
    sys.base_prefix = _runtime_root
    sys.base_exec_prefix = _runtime_root
    sys.executable = os.path.join(_runtime_root, "python.exe")
    if hasattr(sys, "_base_executable"):
        sys._base_executable = sys.executable

_data_root = os.path.abspath(os.environ.get("INDEXTTS_DATA_DIR", "."))
os.makedirs(_data_root, exist_ok=True)
_console_path = os.path.join(_data_root, "engine-console.log")
try:
    if os.path.getsize(_console_path) > 16 * 1024 * 1024:
        try:
            os.replace(_console_path, _console_path + ".old")
        except OSError:
            pass
except OSError:
    pass
try:
    _console_log = open(
        _console_path,
        "a",
        encoding="utf-8",
        errors="backslashreplace",
        buffering=1,
    )
except Exception:
    _console_log = open(os.devnull, "w", encoding="utf-8")


class _SafeTee:
    """调试模式安全双写；控制台失效时仍保留文件日志。"""

    encoding = "utf-8"
    errors = "backslashreplace"

    def __init__(self, *streams):
        self.streams = tuple(stream for stream in streams if stream is not None)

    def write(self, text):
        written = 0
        for stream in self.streams:
            try:
                result = stream.write(text)
                if isinstance(result, int):
                    written = max(written, result)
            except Exception:
                pass
        return written or len(text)

    def flush(self):
        for stream in self.streams:
            try:
                stream.flush()
            except Exception:
                pass

    def isatty(self):
        for stream in self.streams:
            try:
                if stream.isatty():
                    return True
            except Exception:
                pass
        return False

    def fileno(self):
        for stream in self.streams:
            try:
                return stream.fileno()
            except Exception:
                pass
        raise OSError("没有可用的控制台文件描述符")


if os.environ.get("INDEXTTS_DEBUG_CONSOLE", "0") == "1":
    sys.stdout = _SafeTee(_console_log, _original_stdout)
    sys.stderr = _SafeTee(_console_log, _original_stderr)
else:
    sys.stdout = _console_log
    sys.stderr = _console_log

_tts = None
_num_beams = max(1, int(os.environ.get("INDEXTTS_NUM_BEAMS", "1")))
_diffusion_steps = max(1, int(os.environ.get("INDEXTTS_DIFFUSION_STEPS", "16")))
_inference_cfg_rate = float(os.environ.get("INDEXTTS_INFERENCE_CFG_RATE", "0.7"))


def init(root):
    """加载唯一的 PyTorch/ROCm 模型实例。"""
    global _tts, _crash_log
    if _tts is not None:
        return _info()

    root = os.path.abspath(root)
    repo_root = os.path.join(root, "repo")
    if repo_root not in sys.path:
        sys.path.insert(0, repo_root)

    try:
        import faulthandler

        _crash_log = open(
            os.path.join(_data_root, "embed_crash.log"),
            "a",
            encoding="utf-8",
        )
        faulthandler.enable(_crash_log)
    except Exception:
        pass

    checkpoints = os.path.join(_data_root, "checkpoints")
    config_yaml = os.path.join(checkpoints, "config.yaml")
    try:
        from indextts.infer_v2 import IndexTTS2

        _tts = IndexTTS2(
            cfg_path=config_yaml,
            model_dir=checkpoints,
            use_fp16=os.environ.get("INDEXTTS_FP16", "1") == "1",
            use_cuda_kernel=False,
            use_deepspeed=False,
            use_accel=False,
            use_torch_compile=False,
            use_vocoder_fp16=os.environ.get("INDEXTTS_VOCODER_FP16", "1") == "1",
        )
    except Exception:
        import traceback

        raise RuntimeError("IndexTTS2 模型加载失败:\n" + traceback.format_exc())
    return _info()


def _info():
    return {
        "device": str(_tts.device),
        "bridge": "rust-control-plane",
        "streaming": True,
    }


def analyze_emotion(text):
    """仅在 Rust 情绪缓存未命中时调用 Qwen，返回固定 8 维向量。"""
    if _tts is None:
        raise RuntimeError("引擎未初始化")
    values = list(_tts.qwen_emo.inference(str(text)).values())
    if len(values) != 8:
        raise RuntimeError("Qwen 情绪向量维度不是 8")
    return [float(value) for value in values]


def synth_stream(
    text,
    spk_audio_prompt,
    emo_vector=None,
    emo_weight=0.6,
    max_text_tokens_per_segment=120,
):
    """
    逐段返回 `(sample_rate, pcm16_bytes)`。

    WAV 头、临时文件、原子提交和取消都由 Rust 处理。
    """
    if _tts is None:
        raise RuntimeError("引擎未初始化")
    text = (text or "").strip()
    if not text:
        raise ValueError("text 参数为空")
    spk_audio_prompt = os.path.abspath(str(spk_audio_prompt))
    if not os.path.isfile(spk_audio_prompt):
        raise FileNotFoundError("音色文件不存在: " + spk_audio_prompt)

    vector = None
    if emo_vector is not None:
        vector = [float(value) for value in emo_vector]
        if len(vector) != 8:
            raise ValueError("emo_vector 必须是 8 维")

    generator = _tts.infer(
        spk_audio_prompt=spk_audio_prompt,
        text=text,
        output_path=None,
        emo_alpha=max(0.0, min(1.0, float(emo_weight))),
        emo_vector=vector,
        use_emo_text=False,
        use_random=False,
        interval_silence=200,
        max_text_tokens_per_segment=int(max_text_tokens_per_segment),
        num_beams=_num_beams,
        diffusion_steps=_diffusion_steps,
        inference_cfg_rate=_inference_cfg_rate,
        stream_return=True,
    )

    # infer_v2 在 yield 前已经把张量限制到 int16 数值范围并移到 CPU。
    for wav in generator:
        if wav is None:
            continue
        array = wav.detach().cpu().numpy().reshape(-1)
        pcm = array.astype("<i2", copy=True).tobytes()
        if pcm:
            yield 22050, pcm
