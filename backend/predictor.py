from transformers.models.bert import BertTokenizer, BertForSequenceClassification
import torch
import os
import json
from pathlib import Path
from typing import Optional


class EmotionClassifier:
    def __init__(self, model_path_str: Optional[str] = None):
        """加载12类情绪分类模型"""
        # 加载模型和分词器
        model_path_str = model_path_str or os.environ.get(
            "EMOTION_MODEL_PATH", "./emotion_model_12emo"
        )
        model_path = Path(model_path_str).resolve()
        self.tokenizer = BertTokenizer.from_pretrained(
            model_path, local_files_only=True
        )
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.model = torch.nn.Module.to(
            BertForSequenceClassification.from_pretrained(model_path), self.device
        )

        # 从保存的配置加载标签映射
        config_path = os.path.join(model_path, "label_mapping.json")
        with open(config_path, "r", encoding="utf-8") as f:
            label_config = json.load(f)
        self.id2label = label_config["id2label"]
        self.label2id = label_config["label2id"]

        # 打印加载的标签映射
        print("\n加载的标签映射关系:")
        for id, label in self.id2label.items():
            print(f"{id}: {label}")

    def predict(self, text, confidence_threshold=0.08):
        """预测文本情绪（带置信度阈值过滤）"""
        # 编码输入
        inputs = self.tokenizer(
            text, truncation=True, max_length=128, return_tensors="pt"
        ).to(self.device)

        # 推理
        with torch.no_grad():
            outputs = self.model(**inputs)
            probs = torch.softmax(outputs.logits, dim=1)

        # 处理结果
        pred_prob, pred_id = torch.max(probs, dim=1)
        pred_prob = pred_prob.item()
        pred_id = pred_id.item()

        # 获取Top3结果
        top3 = self._get_top3(probs)

        # 低置信度处理
        if pred_prob < confidence_threshold:
            return {
                "label": "不确定",
                "confidence": pred_prob,
                "top3": top3,
                "warning": f"置信度低于阈值({confidence_threshold:.0%})",
            }

        return {
            "label": self.id2label[str(pred_id)],
            "confidence": pred_prob,
            "top3": top3,
        }

    def _get_top3(self, probs):
        """获取概率最高的3个结果"""
        top3_probs, top3_ids = torch.topk(probs, 3)
        return [
            {"label": self.id2label[str(idx.item())], "probability": prob.item()}
            for prob, idx in zip(top3_probs[0], top3_ids[0])
        ]


def main():
    print("【8类情绪分类器】")
    print("=" * 40)
    print("情绪类别: 高兴, 厌恶, 害羞, 害怕, 生气, 认真, 紧张, 慌张")
    print("=" * 40)

    # 初始化分类器
    try:
        classifier = EmotionClassifier()
        print("\n模型加载成功！输入文本进行分析，输入 ':q' 退出")
    except Exception as e:
        print(f"\n模型加载失败: {str(e)}")
        print("请检查：")
        print("1. 模型路径 ./emotion_model_8emo 是否存在")
        print("2. 目录是否包含 label_mapping.json 文件")
        return

    while True:
        try:
            text = input("\n请输入要分析的文本: ").strip()

            # 退出命令
            if text.lower() in [":q", ":quit", "exit"]:
                print("\n退出程序")
                break

            # 空输入处理
            if not text:
                print("输入不能为空！")
                continue

            # 预测并打印结果
            result = classifier.predict(text)
            print("\n" + "=" * 30)
            print(f"📝 文本: {text}")

            if "warning" in result:
                print(f"⚠️ {result['warning']}")

            print(f"🎯 主情绪: {result['label']} (置信度: {result['confidence']:.2%})")

            if result["label"] != "不确定":
                print("\n其他可能情绪:")
                for i, item in enumerate(result["top3"][1:], 1):
                    print(f"{i}. {item['label']}: {item['probability']:.2%}")

            print("=" * 30)

        except KeyboardInterrupt:
            print("\n检测到中断，退出程序...")
            break
        except Exception as e:
            print(f"\n❌ 预测时发生错误: {str(e)}")


if __name__ == "__main__":
    main()
