import os
from fastapi import APIRouter, Body, HTTPException
from fastapi.responses import FileResponse, JSONResponse
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path

router = APIRouter(prefix="/api/v1/chat/background", tags=["Chat Character"])


@router.get("/background_file/{background_file}")
async def get_specific_avatar(background_file: str):
    file_path = os.path.join("game_data", "backgrounds", background_file)
    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="Background not found")

    return FileResponse(file_path)


@router.get("/list")
async def list_all_backgrounds():
    try:
        backgrounds_dir = os.path.join("game_data", "backgrounds")
        if not os.path.exists(backgrounds_dir):
            return {"data": [], "message": "背景图片的目录没有找到"}

        background_files = []
        for filename in os.listdir(backgrounds_dir):
            if filename.lower().endswith(('.png', '.jpg', '.jpeg', '.gif', '.bmp')):
                file_path = os.path.join(backgrounds_dir, filename)
                stat = os.stat(file_path)

                title = os.path.splitext(filename)[0]

                background_files.append({
                    "image_path": filename,
                    "title": title,
                    "modified_time": stat.st_mtime
                })

        background_files.sort(key=lambda x: x["modified_time"], reverse=True)

        if not background_files:
            return {"data": [], "message": "背景图片一个都没找到"}

        return {"data": background_files}

    except Exception as e:
        logger.error(f"获取背景列表失败: {str(e)}")
        return JSONResponse(status_code=500, content={"message": "获取背景列表失败"})