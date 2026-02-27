import csv
from pathlib import Path
from sqlmodel import Session, select

from ling_chat.game_database.database import engine
from ling_chat.game_database.models import Line


def export_emotion_labels(output_path: str | Path | None = None) -> tuple[str, int]:
    """
    从数据库导出情感标签数据到 CSV 文件

    Args:
        output_path: 输出文件路径，默认为当前工作目录下的 ./emotion_labels.csv

    Returns:
        tuple: (生成的 CSV 文件路径，导出条数)
    """
    # 确定输出路径
    if output_path is None:
        output_path = "./emotion_labels.csv"

    output_path = Path(output_path)

    # 连接数据库并查询
    with Session(engine) as session:
        lines = session.exec(select(Line)).all()

        # 写入 CSV
        with open(output_path, 'w', newline='', encoding='utf-8') as f:
            writer = csv.writer(f)
            writer.writerow(['text', 'label'])

            count = 0
            # 用于跟踪已导出的数据对，避免重复
            exported_pairs = set()
            
            for line in lines:
                # 只导出两个情绪字段都不为空且不相同的记录
                if (line.original_emotion and line.predicted_emotion and 
                    line.original_emotion != line.predicted_emotion):
                    # 创建数据对元组
                    data_pair = (line.original_emotion, line.predicted_emotion)
                    # 检查是否已经导出过这个数据对
                    if data_pair not in exported_pairs:
                        writer.writerow([line.original_emotion, line.predicted_emotion])
                        exported_pairs.add(data_pair)
                        count += 1

    return str(output_path), count