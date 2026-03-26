from fastapi import APIRouter, HTTPException, Body
from pydantic import BaseModel
from pathlib import Path
import json
from typing import List, Optional
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.runtime_path import user_data_path

router = APIRouter(prefix="/api/v1/chat/scene", tags=["Chat Scene"])

SCENES_JSON = user_data_path / "game_data" / "backgrounds" / "scenes.json"

class SceneInfo(BaseModel):
    sceneName: str
    sceneImage: str
    sceneDescription: str

def load_scenes_data() -> List[dict]:
    if not SCENES_JSON.exists():
        SCENES_JSON.parent.mkdir(parents=True, exist_ok=True)
        with open(SCENES_JSON, "w", encoding="utf-8") as f:
            json.dump([], f)
        return []
    with open(SCENES_JSON, "r", encoding="utf-8") as f:
        return json.load(f)

def save_scenes_data(scenes: List[dict]):
    with open(SCENES_JSON, "w", encoding="utf-8") as f:
        json.dump(scenes, f, ensure_ascii=False, indent=2)

@router.get("/list")
async def list_scenes():
    """获取所有已保存的场景信息"""
    return {"scenes": load_scenes_data()}

@router.post("/save")
async def save_scene(scene: SceneInfo):
    """保存或更新场景信息"""
    scenes = load_scenes_data()
    # 查找是否已存在同名场景
    existing = next((s for s in scenes if s["sceneName"] == scene.sceneName), None)
    if existing:
        existing.update(scene.dict())
    else:
        scenes.append(scene.dict())
    save_scenes_data(scenes)
    return {"status": "ok"}

@router.post("/delete")
async def delete_scene(sceneName: str = Body(..., embed=True)):
    """删除场景"""
    scenes = load_scenes_data()
    new_scenes = [s for s in scenes if s["sceneName"] != sceneName]
    save_scenes_data(new_scenes)
    return {"status": "ok"}

@router.post("/load")
async def load_scene(
    sceneName: str = Body(..., embed=True),
    immediate: bool = Body(False, embed=True)
):
    """切换场景"""
    scenes = load_scenes_data()
    scene = next((s for s in scenes if s["sceneName"] == sceneName), None)
    if not scene:
        raise HTTPException(status_code=404, detail="场景不存在")

    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=500, detail="AI服务未初始化")

    # 切换背景并设置场景感知台词
    await ai_service.set_scene_info(scene["sceneName"], scene["sceneDescription"], scene["sceneImage"], immediate)
    return {"status": "ok"}

@router.post("/clear")
async def clear_scene():
    """清除当前场景感知"""
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=500, detail="AI服务未初始化")
    await ai_service.clear_scene()
    return {"status": "ok"}
