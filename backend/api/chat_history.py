# api/chat_history.py

from fastapi import APIRouter, Query, HTTPException
from typing import List
from database.user_model import UserConversationModel

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
