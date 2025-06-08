from fastapi import APIRouter
from core.service_manager import service_manager

router = APIRouter(prefix="/api/v1/chat/info", tags=["Chat Info"])

@router.get("/names")
async def list_user_conversations(user_id: int):
    ai_service = service_manager.ai_service
    try:
        result = {
            "ai_name": ai_service.ai_name,
            "ai_subtitle": ai_service.ai_subtitle,
            "user_name": ai_service.user_name,
            "user_subtitle": ai_service.user_subtitle
        }
        return {
            "code": 200,
            "data": result
        }
    except Exception as e:
        return {
            "code": 500,
            "msg": "Failed to fetch user info",
            "error": str(e)
        }