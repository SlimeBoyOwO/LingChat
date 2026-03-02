from pathlib import Path
from ling_chat.utils.runtime_path import user_data_path

SCENES_DIR = user_data_path / "game_data" / "backgrounds"
IMAGE_EXTENSIONS = {'.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp'}

def find_image_for_scene(base_name: str) -> str | None:
    """查找与 base_name 同名的图片文件（不区分扩展名大小写）"""
    for ext in IMAGE_EXTENSIONS:
        # 同时匹配小写和大写扩展名
        for actual_ext in [ext, ext.upper()]:
            img_path = SCENES_DIR / f"{base_name}{actual_ext}"
            if img_path.exists():
                return img_path.name
    return None

def get_scene_description(scene_filename: str) -> str | None:
    """获取场景描述（保持不变）"""
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
        try:
            return scene_path.read_text(encoding='utf-8').strip()
        except Exception:
            return scene_path.stem
    else:
        return None

def list_available_scenes():
    """列出所有可用场景，返回包含 filename、description 和 imageUrl 的字典列表"""
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
            "imageUrl": f"/api/v1/chat/background/background_file/{png.name}"  # 图片本身
        })

    # 处理剩余的 .txt 文件（纯文本场景）
    for txt in txt_files:
        description = txt.read_text(encoding='utf-8').strip()
        # 查找是否存在同名的图片
        image_filename = find_image_for_scene(txt.stem)
        image_url = f"/api/v1/chat/background/background_file/{image_filename}" if image_filename else None
        scenes.append({
            "filename": txt.name,
            "description": description,
            "imageUrl": image_url
        })

    return scenes


