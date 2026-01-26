from fastapi import APIRouter
from ling_chat.core.achievement_manager import achievement_manager
from ling_chat.core.logger import logger

router = APIRouter(prefix="/api/v1/chat/achievement", tags=["Chat Achievement"])

@router.get("/list")
async def get_achievement_list():
    try:
        all_achievements = achievement_manager.get_all_achievements()
        return {"data": all_achievements}
    except Exception as e:
        logger.error(f"获取成就列表失败: {e}")
        return {"data": [], "message": "获取成就列表失败"}
