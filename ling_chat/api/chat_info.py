from fastapi import APIRouter
from ling_chat.core.service_manager import service_manager
import traceback

router = APIRouter(prefix="/api/v1/chat/info", tags=["Chat Info"])


@router.get("/init")
async def init_web_infos(client_id:str ,user_id: int):
    ai_service = service_manager.ai_service
    try:
        # 假如说ai_service没有被初始化，那么就为它初始化
        if not ai_service:
            ai_service = service_manager.init_ai_service()
        
        await service_manager.add_client(client_id)

        result = {
            "ai_name": ai_service.ai_name,
            "ai_subtitle": ai_service.ai_subtitle,
            "user_name": ai_service.user_name,
            "user_subtitle": ai_service.user_subtitle,
            "character_id": ai_service.character_id,
            "clothes_name": ai_service.clothes_name,
            "clothes": ai_service.clothes,
            "thinking_message": ai_service.settings.get("thinking_message", "灵灵正在思考中..."),
            "scale": ai_service.settings.get("scale", 1.0),
            "offset": ai_service.settings.get("offset", 0),
            "bubble_top": ai_service.settings.get("bubble_top", 5),
            "bubble_left": ai_service.settings.get("bubble_left", 20),
            "body_part": ai_service.body_part,
        }
        return {
            "code": 200,
            "data": result
        }
    except Exception as e:
        print("出错了,")
        print(e)
        traceback.print_exc()  # 这会打印完整的错误堆栈到控制台
        return {
            "code": 500,
            "msg": "Failed to fetch user info",
            "error": str(e)
        }
