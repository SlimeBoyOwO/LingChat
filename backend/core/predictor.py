from transformers import BertTokenizer, BertForSequenceClassification
import torch
import os
import json
from pathlib import Path
from .logger import logger, TermColors

class EmotionClassifier:
    def __init__(self, model_path=None):
        """加载情绪分类模型"""
    
        try:
            model_path = model_path or os.environ.get("EMOTION_MODEL_PATH", "./emotion_model_18emo")
            model_path = Path(model_path).resolve()
            self.tokenizer = BertTokenizer.from_pretrained(model_path, local_files_only=True)
            self.model = BertForSequenceClassification.from_pretrained(model_path)
            self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
            self.model.to(self.device)
            
            config_path = os.path.join(model_path, "label_mapping.json")
            with open(config_path, "r", encoding='utf-8') as f: 
                label_config = json.load(f)
            self.id2label = label_config["id2label"]
            self.label2id = label_config["label2id"]
            
            self._log_label_mapping()
            self._log_emotion_model_status(True, f"已成功加载情绪分类模型: {model_path.name}")
        except Exception as e:
            self._log_emotion_model_status(False, f"加载情绪分类模型失败: {e}")
            self.id2label = {}
            self.label2id = {}

    def _log_label_mapping(self):
        """记录标签映射关系"""
        logger.debug("\n加载的标签映射关系:")
        for id, label in self.id2label.items():
            logger.debug(f"{id}: {label}")

    def _log_emotion_model_status(self, is_success: bool, details: str = None):
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

    def predict(self, text, confidence_threshold=0.08):
        """预测文本情绪（带置信度阈值过滤）"""
        try:
            inputs = self.tokenizer(
                text, 
                truncation=True, 
                max_length=128, 
                return_tensors="pt"
            ).to(self.device)
            
            with torch.no_grad():
                outputs = self.model(**inputs)
                probs = torch.softmax(outputs.logits, dim=1)
            
            pred_prob, pred_id = torch.max(probs, dim=1)
            pred_prob = pred_prob.item()
            pred_id = pred_id.item()
            
            top3 = self._get_top3(probs)
            
            if pred_prob < confidence_threshold:
                logger.debug(f"情绪识别置信度低: {text} -> 不确定 ({pred_prob:.2%})")
                return {
                    "label": "不确定",
                    "confidence": pred_prob,
                    "top3": top3,
                    "warning": f"置信度低于阈值({confidence_threshold:.0%})"
                }
            
            label = self.id2label.get(str(pred_id), "")
            logger.debug(f"情绪识别: {text} -> {label} ({pred_prob:.2%})")
            return {
                "label": label,
                "confidence": pred_prob,
                "top3": top3
            }
        except Exception as e:
            logger.error(f"情绪预测错误: {e}")
            return {
                "label": "",
                "confidence": 0.0,
                "top3": [],
                "error": str(e)
            }

    def _get_top3(self, probs):
        """获取概率最高的3个结果"""
        top3_probs, top3_ids = torch.topk(probs, 3)
        return [
            {
                "label": self.id2label.get(str(idx.item()), ""),
                "probability": prob.item()
            }
            for prob, idx in zip(top3_probs[0], top3_ids[0])
        ]

def main():
    print("【情绪分类器】")
    print("="*40)
    
    try:
        classifier = EmotionClassifier()
        print("\n模型加载成功！输入文本进行分析，输入 ':q' 退出")
    except Exception as e:
        print(f"\n模型加载失败: {str(e)}")
        print("请检查：")
        print("1. 模型路径是否存在")
        print("2. 目录是否包含 label_mapping.json 文件")
        return
    
    while True:
        try:
            text = input("\n请输入要分析的文本: ").strip()
            
            if text.lower() in [':q', ':quit', 'exit']:
                print("\n退出程序")
                break
                
            if not text:
                print("输入不能为空！")
                continue
                
            result = classifier.predict(text)
            print("\n" + "="*30)
            print(f"📝 文本: {text}")
            
            if "warning" in result:
                print(f"⚠️ {result['warning']}")
            
            print(f"🎯 主情绪: {result['label']} (置信度: {result['confidence']:.2%})")
            
            if result['label'] != "不确定":
                print("\n其他可能情绪:")
                for i, item in enumerate(result["top3"][1:], 1):
                    print(f"{i}. {item['label']}: {item['probability']:.2%}")
            
            print("="*30)
            
        except KeyboardInterrupt:
            print("\n检测到中断，退出程序...")
            break
        except Exception as e:
            print(f"\n❌ 预测时发生错误: {str(e)}")

if __name__ == "__main__":
    main()
