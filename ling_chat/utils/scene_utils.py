from pathlib import Path
from ling_chat.utils.runtime_path import user_data_path

SCENES_DIR = user_data_path / "game_data" / "backgrounds"

def get_scene_description(scene_filename: str) -> str | None:
    """
    获取场景描述：
    - 如果存在同名的 .txt 文件，读取其内容作为描述；
    - 否则返回文件名（不含扩展名）作为描述。
    - 如果场景图片文件不存在，返回 None。
    """
    scene_path = SCENES_DIR / scene_filename
    if not scene_path.exists():
        return None

    desc_path = scene_path.with_suffix('.txt')
    if desc_path.exists():
        try:
            return desc_path.read_text(encoding='utf-8').strip()
        except Exception:
            # 读取失败时降级使用文件名
            return scene_path.stem
    else:
        return scene_path.stem

def list_available_scenes():
    """列出所有可用场景，返回包含 filename 和 description 的字典列表（不含预览 URL）"""
    if not SCENES_DIR.exists():
        return []
    scenes = []
    for file in SCENES_DIR.glob("*.png"):
        description = get_scene_description(file.name)
        scenes.append({
            "filename": file.name,
            "description": description,
        })
    return scenes