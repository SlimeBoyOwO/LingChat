from pathlib import Path
from typing import List, Optional, Dict, Any
import json
from datetime import datetime
from ling_chat.utils.runtime_path import user_data_path

SCENES_JSON_PATH = user_data_path / "game_data" / "backgrounds" / "backgrounds_scenes.json"

class SceneManager:
    """场景管理器：负责 JSON 场景的 CRUD 操作"""

    def __init__(self):
        self.scenes_file = SCENES_JSON_PATH
        self._ensure_file_exists()

    def _ensure_file_exists(self):
        """确保 JSON 文件存在"""
        if not self.scenes_file.exists():
            self.scenes_file.parent.mkdir(parents=True, exist_ok=True)
            self._save_data({"scenes": []})

    def _load_data(self) -> Dict[str, Any]:
        """加载 JSON 数据"""
        try:
            with open(self.scenes_file, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, FileNotFoundError):
            return {"scenes": []}

    def _save_data(self, data: Dict[str, Any]):
        """保存 JSON 数据"""
        with open(self.scenes_file, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    def create_scene(
        self,
        scene_name: str,
        scene_image: Optional[str],
        scene_description: str
    ) -> Dict[str, Any]:
        """创建新场景"""
        data = self._load_data()
        scene_id = f"scene_{int(datetime.now().timestamp() * 1000)}"
        now = datetime.now().isoformat()

        scene = {
            "id": scene_id,
            "sceneName": scene_name,
            "sceneImage": scene_image,
            "sceneDescription": scene_description,
            "createdAt": now,
            "updatedAt": now
        }

        data["scenes"].append(scene)
        self._save_data(data)
        return scene

    def update_scene(self, scene_id: str, **kwargs) -> Optional[Dict[str, Any]]:
        """更新场景"""
        data = self._load_data()
        for scene in data["scenes"]:
            if scene["id"] == scene_id:
                scene.update(kwargs)
                scene["updatedAt"] = datetime.now().isoformat()
                self._save_data(data)
                return scene
        return None

    def delete_scene(self, scene_id: str) -> bool:
        """删除场景"""
        data = self._load_data()
        original_len = len(data["scenes"])
        data["scenes"] = [s for s in data["scenes"] if s["id"] != scene_id]
        if len(data["scenes"]) < original_len:
            self._save_data(data)
            return True
        return False

    def get_scene(self, scene_id: str) -> Optional[Dict[str, Any]]:
        """获取单个场景"""
        data = self._load_data()
        for scene in data["scenes"]:
            if scene["id"] == scene_id:
                return scene
        return None

    def list_scenes(self) -> List[Dict[str, Any]]:
        """列出所有场景"""
        data = self._load_data()
        return data["scenes"]
