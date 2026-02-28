# ling_chat/api/chat_scene.py
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel
from pathlib import Path
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.runtime_path import user_data_path

router = APIRouter(prefix="/api/v1/chat/scene", tags=["Chat Scene"])

class SceneLoadRequest(BaseModel):
    scene_filename: str

@router.get("/list")
async def list_scenes():
    """获取所有可用场景（背景图片）及其描述"""
    scenes_dir = user_data_path / "game_data" / "backgrounds"
    if not scenes_dir.exists():
        return {"scenes": []}
    scenes = []
    for file in scenes_dir.glob("*.png"):
        desc_path = file.with_suffix('.txt')
        description = desc_path.read_text(encoding='utf-8').strip() if desc_path.exists() else file.stem
        scenes.append({
            "filename": file.name,
            "description": description,
            "preview": f"/api/v1/chat/background/background_file/{file.name}"  # 复用已有背景图片获取接口
        })
    return {"scenes": scenes}

@router.post("/load")
async def load_scene(request: SceneLoadRequest):
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=500, detail="AI服务未初始化")
    success = await ai_service.set_scene(request.scene_filename)
    if not success:
        raise HTTPException(status_code=404, detail="场景文件不存在")
    return {"status": "ok"}

@router.post("/clear")
async def clear_scene():
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=500, detail="AI服务未初始化")
    await ai_service.clear_scene()
    return {"status": "ok"}