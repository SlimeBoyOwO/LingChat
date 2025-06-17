import os
from fastapi import APIRouter
from core.service_manager import service_manager
from database.user_model import UserModel
from database.character_model import CharacterModel
from utils.function import Function

router = APIRouter(prefix="/api/v1/chat/info", tags=["Chat Info"])
    
@router.get("/init")
async def init_web_infos(user_id: int):
    ai_service = service_manager.ai_service
    try:
        # 假如说ai_service没有被初始化，那么就为它初始化
        if ai_service is None:
            # dict の 两种访问方式
            user_info = UserModel.get_user_by_id(user_id=user_id)
            if user_info is None:
                UserModel.create_user(username="admin", password="114514")
                user_info = UserModel.get_user_by_id(user_id=user_id)
            last_character_id = user_info.get("last_chat_character")
            resource = CharacterModel.get_character_by_id(last_character_id)["resource_path"]

            settings = Function.parse_enhanced_txt(os.path.join(resource,"settings.txt"))
            settings["character_id"] = last_character_id
            ai_service = service_manager.init_ai_service(settings)
            
        result = {
            "ai_name": ai_service.ai_name,
            "ai_subtitle": ai_service.ai_subtitle,
            "user_name": ai_service.user_name,
            "user_subtitle": ai_service.user_subtitle,
            "character_id": ai_service.character_id,
            "scale": ai_service.settings.get("scale",1.0),
            "offset": ai_service.settings.get("offset",0)
        }
        return {
            "code": 200,
            "data": result
        }
    except Exception as e:
        print("出错了,")
        print(e)
        return {
            "code": 500,
            "msg": "Failed to fetch user info",
            "error": str(e)
        }
    

