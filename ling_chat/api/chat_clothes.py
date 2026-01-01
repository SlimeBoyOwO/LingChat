import os
from pathlib import Path
from fastapi import APIRouter, Body, HTTPException
from fastapi.responses import FileResponse, JSONResponse
from ling_chat.core.service_manager import service_manager
from ling_chat.database.character_model import CharacterModel
from ling_chat.database.user_model import UserModel
from ling_chat.utils.function import Function
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import user_data_path
from typing import List

router = APIRouter(prefix="/api/v1/chat/clothes", tags=["Chat Clothes"])

@router.get("/list")
async def list_all_clothes():
    ai_service = service_manager.ai_service

    if not ai_service or not ai_service.character_path:
        raise HTTPException(status_code=404, detail="AIService or character_path not found")

    file_path = Path(ai_service.character_path) / "avatar"
    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="Avatar not found")
    
    clothes_list = []
    for item in file_path.iterdir():
        if item.is_dir():
            clothes_list.append({
                "title": item.name,
                "avatar": str(item)
            })
    return JSONResponse(content=clothes_list)

@router.get("/clothes_file/{file_path:path}")
async def get_clothes_file(file_path: str):

    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="文件不存在")

    return FileResponse(file_path)

@router.get("/open_web")
async def open_creative_web():
    try:
        import webbrowser
        url = "https://github.com/SlimeBoyOwO/LingChat/discussions"
        webbrowser.open(url)
    except Exception as e:
        logger.error(f"无法使用浏览器启动创意工坊: {str(e)}")
        raise HTTPException(status_code=500, detail="无法使用浏览器启动网页")