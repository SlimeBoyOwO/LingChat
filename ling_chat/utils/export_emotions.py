import csv
from pathlib import Path

from sqlmodel import Session, select

from ling_chat.game_database.database import engine
from ling_chat.game_database.models import Line


def export_emotion_labels(
    output_path: str | Path | None = None, full_export: bool = False
) -> tuple[str, int]:
    """
    从数据库导出情感标签数据到 CSV 文件

    Args:
        output_path: 输出文件路径，默认为当前工作目录下的 ./emotion_labels.csv
        full_export: 是否启用全部导出模式。如果为True，则导出所有记录的sentence、text和label，
                    不进行情感排除，去重基于sentence字段

    Returns:
        tuple: (生成的 CSV 文件路径，导出条数)
    """
    if full_export:
        headers = ["sentence", "text", "label"]
    else:
        headers = ["text", "label"]

    # 定义要排除的情感列表
    excluded_emotions = (
        "慌张",
        "担心",
        "尴尬",
        "紧张",
        "高兴",
        "自信",
        "害怕",
        "害羞",
        "认真",
        "生气",
        "无语",
        "厌恶",
        "疑惑",
        "难为情",
        "惊讶",
        "情动",
        "哭泣",
        "调皮",
        "平静",
    )

    # 确定输出路径
    if output_path is None:
        output_path = "./emotion_labels.csv"

    output_path = Path(output_path)

    # 连接数据库并查询
    with Session(engine) as session:
        lines = session.exec(select(Line)).all()

        # 写入 CSV
        with open(output_path, "w", newline="", encoding="utf-8") as f:
            writer = csv.writer(f)
            writer.writerow(headers)

            count = 0

            if full_export:
                # 全部导出模式：去重基于sentence，不进行情感排除
                exported_sentences = set()

                for line in lines:
                    if (
                        line.content
                        and line.original_emotion
                        and line.predicted_emotion
                    ):
                        # 检查是否已经导出过这个sentence
                        if line.content not in exported_sentences:
                            writer.writerow(
                                [
                                    line.content,
                                    line.original_emotion,
                                    line.predicted_emotion,
                                ]
                            )
                            exported_sentences.add(line.content)
                            count += 1
            else:
                # 默认模式：去重基于text，进行情感排除
                exported_texts = set()

                for line in lines:
                    # 只要 original_emotion 或 predicted_emotion 不在排除列表中就导出
                    # 即使两个字段相同也可以导出
                    if (
                        line.original_emotion
                        and line.predicted_emotion
                        and (
                            line.original_emotion not in excluded_emotions
                            or line.predicted_emotion not in excluded_emotions
                        )
                    ):
                        # 检查是否已经导出过这个text
                        if line.original_emotion not in exported_texts:
                            writer.writerow(
                                [line.original_emotion, line.predicted_emotion]
                            )
                            exported_texts.add(line.original_emotion)
                            count += 1

    return str(output_path), count
