# api/chat_history.py

from fastapi import APIRouter, Query, HTTPException
from typing import List
from database.user_model import UserConversationModel
from database.conversation_model import ConversationModel
from core.service_manager import service_manager

router = APIRouter(prefix="/api/v1/chat/history", tags=["Chat History"])

@router.get("/list")
async def list_user_conversations(user_id: int, page: int = 1, page_size: int = 10):
    try:
        result = UserConversationModel.get_user_conversations(user_id, page, page_size)
        return {
            "code": 200,
            "data": result
        }
    except Exception as e:
        return {
            "code": 500,
            "msg": "Failed to fetch user conversations",
            "error": str(e)
        }
    
@router.get("/load")
async def load_user_conversations(user_id: int, conversation_id: int):
    try:
        print("成功调用了api")
        result = ConversationModel.get_conversation_messages(conversation_id=conversation_id)
        print(f"获取的结果是{result}")
        if(result != None):
            service_manager.ai_service.load_memory(result)
            print("成功调用记忆存储")
            return {
                "code": 200,
                "data": "success"
            }
        else:
            return {
                "code": 500,
                "msg": "Failed to load user conversations",
                "error": "加载的数据是空的"
            }
        
    except Exception as e:
        return {
            "code": 500,
            "msg": "Failed to load user conversations",
            "error": str(e)
        }