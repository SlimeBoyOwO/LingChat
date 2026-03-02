from pathlib import Path
from ling_chat.utils.runtime_path import user_data_path

SCENES_DIR = user_data_path / "game_data" / "backgrounds"

def get_scene_description(scene_filename: str) -> str | None:
    """
    获取场景描述：
    - 如果传入的是 .png 文件，检查是否存在同名的 .txt 文件，存在则读取其内容作为描述，否则返回文件名（不含扩展名）。
    - 如果传入的是 .txt 文件，直接读取其内容作为描述。
    - 如果文件不存在，返回 None。
    """
    scene_path = SCENES_DIR / scene_filename
    if not scene_path.exists():
        return None

    if scene_path.suffix.lower() == '.png':
        # 优先找同名的 .txt
        desc_path = scene_path.with_suffix('.txt')
        if desc_path.exists():
            try:
                return desc_path.read_text(encoding='utf-8').strip()
            except Exception:
                return scene_path.stem
        else:
            return scene_path.stem
    elif scene_path.suffix.lower() == '.txt':
        # 直接读取 .txt 内容
        try:
            return scene_path.read_text(encoding='utf-8').strip()
        except Exception:
            return scene_path.stem
    else:
        return None

def list_available_scenes():
    """列出所有可用场景，返回包含 filename 和 description 的字典列表。
    对于 .png 文件，如果存在同名的 .txt 则使用其内容作为描述，否则用文件名。
    对于单独的 .txt 文件，将其视为场景，文件名作为场景名，文件内容作为描述。
    """
    if not SCENES_DIR.exists():
        return []
    scenes = []
    png_files = set(SCENES_DIR.glob("*.png"))
    txt_files = set(SCENES_DIR.glob("*.txt"))

    # 处理 .png 文件
    for png in png_files:
        txt_path = png.with_suffix('.txt')
        if txt_path in txt_files:
            description = txt_path.read_text(encoding='utf-8').strip()
            txt_files.remove(txt_path)
        else:
            description = png.stem
        scenes.append({
            "filename": png.name,
            "description": description,
        })

    # 处理剩余的 .txt 文件（纯文本场景）
    for txt in txt_files:
        description = txt.read_text(encoding='utf-8').strip()
        scenes.append({
            "filename": txt.name,
            "description": description,
        })

    return scenes
