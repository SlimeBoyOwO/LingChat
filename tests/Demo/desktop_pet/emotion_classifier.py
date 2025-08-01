# file: emotion_classifier.py

import json
import torch
from transformers import BertTokenizer, BertForSequenceClassification

from config import CONFIG # Import config for paths and defaults

class EmotionClassifier:
    def __init__(self, model_path, logger_instance):
        self.logger = logger_instance
        self.model = None
        self.tokenizer = None
        self.id2label = {}

        try:
            if not model_path.exists():
                raise FileNotFoundError(f"情绪模型路径不存在: {model_path}")
            if not model_path.is_dir():
                raise ValueError(f"情绪模型路径 '{model_path}' 不是一个目录。")

            self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
            self.logger.info(f"情绪分类模型将使用设备: {self.device}")

            self.tokenizer = BertTokenizer.from_pretrained(str(model_path), local_files_only=True)
            self.model = BertForSequenceClassification.from_pretrained(str(model_path), local_files_only=True)
            self.model.to(self.device)
            self.model.eval()

            mapping_path = model_path / "label_mapping.json"
            if not mapping_path.exists():
                 raise FileNotFoundError(f"找不到标签映射文件: {mapping_path}")
            
            with open(mapping_path, 'r', encoding='utf-8') as f:
                label_config = json.load(f)
            
            if "id2label" in label_config:
                self.id2label = {str(k): v for k, v in label_config["id2label"].items()}
            elif "label2id" in label_config:
                self.id2label = {str(v): k for k, v in label_config["label2id"].items()}

            if not self.id2label:
                raise ValueError("label_mapping.json 格式不正确或为空")
            
            self.logger.info(f"成功加载情绪分类模型: {model_path.name}")
        except Exception as e:
            self.logger.error(f"加载情绪分类模型失败: {e}", exc_info=True)
            self.model = None

    def predict(self, text):
        if not self.model:
            self.logger.warning("情绪模型未加载，无法进行预测。返回默认情绪。")
            return CONFIG["DEFAULT_EMOTION"]

        try:
            inputs = self.tokenizer(text, truncation=True, max_length=128, return_tensors="pt").to(self.device)
            with torch.no_grad():
                outputs = self.model(**inputs)
                logits = outputs.logits
                pred_id = torch.argmax(logits, dim=1).item()
            
            predicted_label = self.id2label.get(str(pred_id), CONFIG["DEFAULT_EMOTION"])
            self.logger.info(f"情绪预测: '{text[:30]}...' -> '{predicted_label}'")
            return predicted_label
        except Exception as e:
            self.logger.error(f"情绪预测时发生错误: {e}")
            return CONFIG["ERROR_EMOTION"]