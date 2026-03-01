from fastapi import APIRouter, HTTPException, Body
from pathlib import Path
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.utils.scene_utils import list_available_scenes   

router = APIRouter(prefix="/api/v1/chat/scene", tags=["Chat Scene"])

@router.get("/list")
async def list_scenes():
    """获取所有可用场景（背景图片）及其描述"""
    scenes = list_available_scenes()
    # 为每个场景添加预览 URL（该路径为 API 特有，因此在 API 层补充）
    for scene in scenes:
        scene["preview"] = f"/api/v1/chat/background/background_file/{scene['filename']}"
    return {"scenes": scenes}

@router.post("/load")
async def load_scene(
    scene_filename: str = Body(..., embed=True)
):
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=500, detail="AI服务未初始化")
    success = await ai_service.set_scene(scene_filename)
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
