from fastapi import APIRouter, HTTPException, Body
from pydantic import BaseModel
from pathlib import Path
from typing import Optional
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.utils.scene_utils import list_available_scenes, SCENES_DIR
from ling_chat.utils.scene_manager import SceneManager
from ling_chat.core.pic_analyzer import DesktopAnalyzer
from ling_chat.core.logger import logger

router = APIRouter(prefix="/api/v1/chat/scene", tags=["Chat Scene"])

class SceneCreateRequest(BaseModel):
    sceneName: str
    sceneImage: Optional[str] = None
    sceneDescription: str = ""
    autoAnalyze: bool = False

class SceneUpdateRequest(BaseModel):
    id: str
    sceneName: Optional[str] = None
    sceneImage: Optional[str] = None
    sceneDescription: Optional[str] = None

class SceneDeleteRequest(BaseModel):
    id: str

class SceneLoadRequest(BaseModel):
    sceneId: str
    triggerAIResponse: bool = False

class SceneIdRequest(BaseModel):
    id: str

@router.post("/create")
async def create_scene(request: SceneCreateRequest):
    """创建新场景"""
    scene_manager = SceneManager()

    # 验证场景名称
    if not request.sceneName.strip():
        raise HTTPException(status_code=400, detail="场景名称不能为空")

    # 如果有图片，验证图片文件存在
    if request.sceneImage:
        image_path = SCENES_DIR / request.sceneImage
        if not image_path.exists():
            raise HTTPException(status_code=404, detail=f"图片文件不存在: {request.sceneImage}")

    # 处理场景描述
    scene_description = request.sceneDescription.strip()

    # 如果需要自动分析图片
    if request.autoAnalyze and request.sceneImage and not scene_description:
        try:
            image_path = SCENES_DIR / request.sceneImage
            analyzer = DesktopAnalyzer()
            scene_description = await analyzer.analyze_image_file(
                image_path,
                prompt="请用100字左右描述这个场景的环境、氛围等特征"
            )
            logger.info(f"自动分析图片生成描述: {scene_description}")
        except Exception as e:
            logger.error(f"图片分析失败: {e}")
            raise HTTPException(status_code=500, detail=f"图片分析失败: {str(e)}")

    # 创建场景
    if not scene_description:
        raise HTTPException(status_code=500, detail=f"图片描述创建失败")
    try:
        scene = scene_manager.create_scene(
            scene_name=request.sceneName.strip(),
            scene_image=request.sceneImage,
            scene_description=scene_description
        )
        return {"status": "ok", "scene": scene}
    except Exception as e:
        logger.error(f"创建场景失败: {e}")
        raise HTTPException(status_code=500, detail=f"创建场景失败: {str(e)}")

@router.put("/update")
async def update_scene(request: SceneUpdateRequest):
    """更新场景"""
    scene_manager = SceneManager()

    # 构建更新数据
    updates = {}
    if request.sceneName is not None:
        updates["sceneName"] = request.sceneName.strip()
    if request.sceneImage is not None:
        updates["sceneImage"] = request.sceneImage
    if request.sceneDescription is not None:
        updates["sceneDescription"] = request.sceneDescription.strip()

    # 更新场景
    scene = scene_manager.update_scene(request.id, **updates)
    if not scene:
        raise HTTPException(status_code=404, detail="场景不存在")

    return {"status": "ok", "scene": scene}

@router.delete("/delete")
async def delete_scene(request: SceneDeleteRequest):
    """删除场景"""
    scene_manager = SceneManager()

    success = scene_manager.delete_scene(request.id)
    if not success:
        raise HTTPException(status_code=404, detail="场景不存在")

    return {"status": "ok", "message": "场景已删除"}

@router.get("/list")
async def list_scenes():
    """获取所有可用场景"""
    scene_manager = SceneManager()

    # 获取 JSON 场景
    json_scenes = scene_manager.list_scenes()

    # 为每个场景生成 imageUrl
    for scene in json_scenes:
        if scene.get("sceneImage"):
            scene["imageUrl"] = f"/api/v1/chat/background/background_file/{scene['sceneImage']}"
        else:
            scene["imageUrl"] = None
        scene["source"] = "json"

    # 获取 .txt 场景（向后兼容）
    txt_scenes = list_available_scenes()

    # 合并场景列表，JSON 优先
    json_filenames = {s.get("sceneImage") for s in json_scenes if s.get("sceneImage")}
    for txt_scene in txt_scenes:
        if txt_scene.get("filename") not in json_filenames:
            # 转换为新格式
            json_scenes.append({
                "id": f"txt_{txt_scene['filename']}",
                "sceneName": txt_scene.get("filename", "").replace(".txt", ""),
                "sceneImage": txt_scene.get("filename"),
                "sceneDescription": txt_scene.get("description", ""),
                "imageUrl": txt_scene.get("imageUrl"),
                "source": "txt",
                "createdAt": "",
                "updatedAt": ""
            })

    return {"scenes": json_scenes}

@router.post("/load")
async def load_scene(request: SceneLoadRequest):
    """加载场景"""
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=500, detail="AI服务未初始化")

    scene_manager = SceneManager()
    scene = scene_manager.get_scene(request.sceneId)

    if not scene:
        raise HTTPException(status_code=404, detail="场景不存在")

    # 调用 AI 服务设置场景
    success = await ai_service.set_scene(
        scene_id=request.sceneId,
        trigger_response=request.triggerAIResponse
    )

    if not success:
        raise HTTPException(status_code=500, detail="场景加载失败")

    return {"status": "ok", "scene": scene}
