import os
import shutil
from pathlib import Path

from fastapi import APIRouter, HTTPException, UploadFile
from fastapi.responses import FileResponse, JSONResponse

from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.runtime_path import user_data_path

router = APIRouter(prefix="/api/v1/chat/background", tags=["Chat Character"])

BACKGROUND_DIR = user_data_path / "game_data/backgrounds"
ALLOWED_EXTENSIONS = {'.jpg', '.png', '.webp', '.bmp', '.svg', '.tif', '.gif'}

@router.get("/background_script_file/{background_file}")
async def get_script_background_file(background_file: str):
    try:

        ai_service = service_manager.ai_service
        if ai_service is None:
            raise HTTPException(status_code=404, detail="AISERVICE not found")
        else:
            file_path =  ai_service.scripts_manager.get_assests_dir() / "Backgrounds" / background_file

        if not os.path.exists(file_path):
            raise HTTPException(status_code=404, detail="Background not found")

        return FileResponse(file_path)
    except Exception as e:
        # 日志记录异常
        print(f"An error occurred: {e}")

@router.get("/background_file/{background_file}")
async def get_specific_avatar(background_file: str):
    file_path = BACKGROUND_DIR / background_file
    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="Background not found")

    return FileResponse(file_path)


@router.get("/list")
async def list_all_backgrounds():
    try:
        if not os.path.exists(BACKGROUND_DIR):
            return {"data": [], "message": "背景图片的目录没有找到"}

        background_files = []
        for f in BACKGROUND_DIR.iterdir():
            filename = f.name
            if f.suffix.lower() in ('.png', '.jpg', '.jpeg', '.gif', '.bmp'):
                file_path = BACKGROUND_DIR / filename
                stat = f.stat()

                title = f.stem  # 使用文件名作为标题

                background_files.append({
                    "title": title,
                    "url": filename,
                    "time": str(stat.st_mtime)
                })

        background_files.sort(key=lambda x: x["time"], reverse=True)

        if not background_files:
            return {"data": [], "message": "背景图片一个都没找到"}

        return {"data": background_files}

    except Exception as e:
        logger.error(f"获取背景列表失败: {str(e)}")
        return JSONResponse(status_code=500, content={"message": "获取背景列表失败"})


@router.post("/upload")
async def upload_music(file: UploadFile, name: str | None = None):
    """
    上传一个背景图片文件到服务器
    """
    try:
        file_ext = Path(file.filename).suffix.lower()
        if file_ext not in ALLOWED_EXTENSIONS:
            raise HTTPException(status_code=400, detail="无效文件类型")

        BACKGROUND_DIR.mkdir(parents=True, exist_ok=True)

        filename = name if name else file.filename
        save_path = BACKGROUND_DIR / filename

        with save_path.open("wb") as buffer:
            shutil.copyfileobj(file.file, buffer)

        return JSONResponse(
            status_code=200,
            content={"message": "背景图片上传成功"}
        )
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"未能上传背景图片: {str(e)}")
