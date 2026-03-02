from fastapi import APIRouter, HTTPException, Body
from pydantic import BaseModel
from pathlib import Path
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.utils.scene_utils import list_available_scenes, SCENES_DIR

router = APIRouter(prefix="/api/v1/chat/scene", tags=["Chat Scene"])

class SceneCreateRequest(BaseModel):
    name: str
    description: str

@router.post("/create")
async def create_scene(request: SceneCreateRequest):
    """创建新的场景描述文件（.txt）"""
    scenes_dir = SCENES_DIR
    scenes_dir.mkdir(parents=True, exist_ok=True)

    # 安全处理：只允许字母、数字、空格、短横线和下划线
    safe_name = "".join(c for c in request.name if c.isalnum() or c in (' ', '-', '_')).strip()
    if not safe_name:
        raise HTTPException(status_code=400, detail="场景名包含非法字符或为空")

    filename = f"{safe_name}.txt"
    file_path = scenes_dir / filename

    if file_path.exists():
        raise HTTPException(status_code=400, detail="场景已存在")

    try:
        file_path.write_text(request.description.strip(), encoding='utf-8')
        return {"status": "ok", "filename": filename}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"写入文件失败: {str(e)}")

@router.get("/list")
async def list_scenes():
    """获取所有可用场景及其描述和图片URL"""
    scenes = list_available_scenes()
    return {"scenes": scenes}   # 注意：此时 scenes 已经包含 imageUrl 字段

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
