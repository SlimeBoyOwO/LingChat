import json
import os
from pathlib import Path

import numpy as np
import onnxruntime as ort

from ling_chat.core.logger import TermColors, logger
from ling_chat.utils.runtime_path import third_party_path


class EmotionClassifier:
    def __init__(self, model_path=None):
        """加载情绪分类模型 (ONNX版本) - 增强兼容性修复版"""

        # 检查是否启用了情感分类器
        if os.environ.get("ENABLE_EMOTION_CLASSIFIER", "True").lower() == "false":
            self._log_emotion_model_status(False, "情绪分类器已禁用")
            self.id2label = {}
            self.label2id = {}
            self.session = None 
            self.vocab = {}
            return

        try:
            model_path = model_path or os.environ.get("EMOTION_MODEL_PATH", third_party_path / "emotion_model")
            model_path = Path(model_path).resolve()

            # 定义文件路径
            onnx_model_file = model_path / "model.onnx"
            config_path = model_path / "label_mapping.json"
            vocab_path = model_path / "vocab.txt"

            # 1. 基础文件检查
            if not onnx_model_file.exists():
                raise FileNotFoundError(f"ONNX模型文件不存在: {onnx_model_file}")
            if not config_path.exists():
                raise FileNotFoundError(f"标签映射文件不存在: {config_path}")
            
            # 加载ONNX模型
            onnx_path = onnx_model_file

            self.session = ort.InferenceSession(onnx_path)

            # 加载标签映射
            label_mapping_path = config_path
            if not os.path.exists(label_mapping_path):
                raise FileNotFoundError(f"标签映射文件不存在: {label_mapping_path}")

            with open(label_mapping_path, "r", encoding="utf-8") as f:
                label_mapping = json.load(f)
                self.id2label = label_mapping["id2label"]
                self.label2id = label_mapping["label2id"]
            
            self.vocab = self._load_vocab(vocab_path)

            # 获取输入输出名称
            self.input_name = self.session.get_inputs()[0].name
            self.attention_mask_name = self.session.get_inputs()[1].name
            self.output_name = self.session.get_outputs()[0].name

            self._log_label_mapping()
            self._log_emotion_model_status(True, f"已成功加载情绪分类ONNX模型: {model_path.name}")

        except Exception as e:
            import traceback
            # 如果是 Unicode 错误，通常意味着底层有其他报错被掩盖了
            if "UnicodeDecodeError" in str(e) or "'utf-8' codec" in str(e):
                logger.error("检测到编码冲突错误。这通常是因为Windows系统区域设置导致的。")
                logger.error("尝试设置环境变量 PYTHONUTF8=1 可能有帮助。")
            
            # 打印完整堆栈
            logger.error(f"加载模型详细错误堆栈:\n{traceback.format_exc()}")
            
            self._log_emotion_model_status(False, f"加载失败: {e}")
            self.id2label = {}
            self.label2id = {}
            self.session = None
            self.vocab = {}

    def _load_vocab(self, vocab_path):
        """从 vocab.txt 加载词汇表"""
        with open(vocab_path, "r", encoding="utf-8") as f:
            vocab = {line.strip(): idx for idx, line in enumerate(f)}
        return vocab

    def _log_label_mapping(self):
        """记录标签映射关系"""
        logger.debug("\n加载的标签映射关系:")
        for id, label in self.id2label.items():
            logger.debug(f"{id}: {label}")

    def _log_emotion_model_status(self, is_success: bool, details: str = ""):
        """情绪模型加载状态记录，兼容旧接口"""
        status = "情绪分类模型加载正常" if is_success else "情绪分类模型加载异常"
        status_color = TermColors.GREEN if is_success else TermColors.RED
        status_symbol = "√" if is_success else "×"

        if details:
            if is_success:
                logger.info(f"{status_color}{status_symbol}{TermColors.RESET} {status} - {details}")
            else:
                logger.error(f"{status_color}{status_symbol}{TermColors.RESET} {status} - {details}")
        else:
            if is_success:
                logger.info(f"{status_color}{status_symbol}{TermColors.RESET} {status}")
            else:
                logger.error(f"{status_color}{status_symbol}{TermColors.RESET} {status}")

    def _tokenize(self, text, max_length=128):
        """手动实现分词、ID转换和填充"""
        tokens = list(text) # 基础的按字分词

        # 转换为ID
        token_ids = [self.vocab.get(token, self.vocab.get("[UNK]")) for token in tokens]

        # 截断
        if len(token_ids) > max_length - 2:
            token_ids = token_ids[:max_length - 2]

        # 添加特殊标记 [CLS] 和 [SEP]
        input_ids = [self.vocab["[CLS]"]] + token_ids + [self.vocab["[SEP]"]]
        attention_mask = [1] * len(input_ids)

        # 填充
        padding_length = max_length - len(input_ids)
        input_ids += [self.vocab["[PAD]"]] * padding_length
        attention_mask += [0] * padding_length

        return {
            "input_ids": np.array([input_ids], dtype=np.int64),
            "attention_mask": np.array([attention_mask], dtype=np.float32),  # 修复: 使用float32类型
        }

    def _softmax(self, x):
        """使用Numpy计算Softmax"""
        exp_x = np.exp(x - np.max(x, axis=-1, keepdims=True))
        return exp_x / np.sum(exp_x, axis=-1, keepdims=True)

    def predict(self, text, confidence_threshold=0.08):
        """预测文本情绪（带置信度阈值过滤）- ONNX版本"""
        # 如果模型未加载（可能被环境变量禁用），直接返回传入的文本作为情感标签
        if not hasattr(self, 'session') or self.session is None:
            return {
                "label": text,
                "confidence": 1.0,
                "top3": [{"label": text, "probability": 1.0}],
                "disabled": True
            }

        # 如果传入的文本已经是有效的情感标签，直接返回而不进行预测
        if text in self.label2id and os.environ.get("ENABLE_DIRECT_EMOTION_CLASSIFIER", "false").lower() == "true":
            logger.debug(f"输入文本 '{text}' 已是有效情感标签，直接返回")
            return {
                "label": text,
                "confidence": 1.0,
                "top3": [{"label": text, "probability": 1.0}]
            }

        try:
            # 手动分词和编码
            inputs = self._tokenize(text, max_length=128)

            # 准备ONNX模型的输入
            ort_inputs = {
                self.input_name: inputs['input_ids'],
                self.attention_mask_name: inputs['attention_mask'],
            }

            # 执行ONNX推理
            ort_outputs = self.session.run([self.output_name], ort_inputs)
            logits = ort_outputs[0]

            # 计算概率
            probs = self._softmax(logits)[0] # 获取第一个（也是唯一一个）结果的概率分布

            pred_id = np.argmax(probs)
            pred_prob = probs[pred_id]

            top3 = self._get_top3(probs)

            if pred_prob < confidence_threshold:
                logger.debug(f"情绪识别置信度低: {text} -> 不确定 ({pred_prob:.2%})")
                return {
                    "label": "不确定",
                    "confidence": float(pred_prob),
                    "top3": top3,
                    "warning": f"置信度低于阈值({confidence_threshold:.0%})"
                }

            label = self.id2label.get(str(pred_id), "")
            logger.debug(f"情绪识别: {text} -> {label} ({pred_prob:.2%})")
            return {
                "label": label,
                "confidence": float(pred_prob),
                "top3": top3
            }
        except Exception as e:
            logger.error(f"情绪预测错误: {e}")
            return {
                "label": text,
                "confidence": 1.0,
                "top3": [{"label": text, "probability": 1.0}],
                "error": str(e)
            }

    def _get_top3(self, probs):
        """获取概率最高的3个结果 - Numpy版本"""
        top3_ids = np.argsort(probs)[-3:][::-1] # 获取概率最高的3个索引
        return [
            {
                "label": self.id2label.get(str(idx), ""),
                "probability": float(probs[idx])
            }
            for idx in top3_ids
        ]

# 实例化部分保持不变，可以直接使用新的 EmotionClassifier 类
emotion_classifier = EmotionClassifier()
