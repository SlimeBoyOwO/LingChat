import os
import json
from pathlib import Path
from ling_chat.core.logger import logger, TermColors
from ling_chat.utils.runtime_path import third_party_path
import onnxruntime as ort
import numpy as np

class EmotionClassifier:
    def __init__(self, model_path=None):
        """加载情绪分类模型 (ONNX版本)"""

        # 检查是否启用了情感分类器
        if os.environ.get("ENABLE_EMOTION_CLASSIFIER", "True").lower() == "false":
            self._log_emotion_model_status(False, "情绪分类器已通过 ENABLE_EMOTION_CLASSIFIER 环境变量禁用，将直接传递情感标签")
            self.id2label = {}
            self.label2id = {}
            self.session = None # 使用 self.session 替代 self.model
            self.vocab = {}
            return

        try:
            model_path = model_path or os.environ.get("EMOTION_MODEL_PATH", third_party_path / "emotion_model_18emo")
            model_path = Path(model_path).resolve()
            
            # 定义 ONNX 模型和其他必要文件的路径
            onnx_model_file = model_path / "model.onnx"
            config_path = model_path / "label_mapping.json"
            vocab_path = model_path / "vocab.txt"

            if not onnx_model_file.exists():
                raise FileNotFoundError(f"ONNX模型文件不存在: {onnx_model_file}")
            if not config_path.exists():
                raise FileNotFoundError(f"标签映射文件不存在: {config_path}")
            if not vocab_path.exists():
                raise FileNotFoundError(f"词汇表文件不存在: {vocab_path}")

            # 加载标签映射
            with open(config_path, "r", encoding='utf-8') as f: 
                label_config = json.load(f)
            self.id2label = label_config["id2label"]
            self.label2id = label_config["label2id"]
            
            # 加载词汇表以进行手动分词
            self.vocab = self._load_vocab(vocab_path)
            
            # 创建ONNX Runtime会话，并指定使用CPU
            providers = ['CPUExecutionProvider']
            self.session = ort.InferenceSession(str(onnx_model_file), providers=providers)
            
            self._log_label_mapping()
            self._log_emotion_model_status(True, f"已成功加载情绪分类ONNX模型: {model_path.name}")
            
        except Exception as e:
            self._log_emotion_model_status(False, f"加载情绪分类ONNX模型失败: {e}")
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
        
        # 创建 token_type_ids
        token_type_ids = [0] * max_length
        
        return {
            "input_ids": np.array([input_ids], dtype=np.int64),
            "attention_mask": np.array([attention_mask], dtype=np.int64),
            "token_type_ids": np.array([token_type_ids], dtype=np.int64)
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
                'input_ids': inputs['input_ids'],
                'attention_mask': inputs['attention_mask'],
                'token_type_ids': inputs['token_type_ids']
            }
            
            # 执行ONNX推理
            ort_outputs = self.session.run(None, ort_inputs)
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