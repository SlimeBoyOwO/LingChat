import shutil
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Dict, List

from fastapi import APIRouter, FastAPI, HTTPException, UploadFile
from fastapi.responses import FileResponse, JSONResponse

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path

TEMPLATE_MUSIC_DIR = static_path / "game_data/musics"
MUSIC_DIR = user_data_path / "game_data/musics"
ALLOWED_EXTENSIONS = {'.mp3', '.wav', '.flac', '.webm', '.weba', '.ogg', '.m4a', '.oga'}


@asynccontextmanager
async def lifespan(app: FastAPI):
    if not MUSIC_DIR.exists():
        if TEMPLATE_MUSIC_DIR.exists():
            shutil.copytree(TEMPLATE_MUSIC_DIR, MUSIC_DIR, dirs_exist_ok=True)
        else:
            MUSIC_DIR.mkdir(parents=True, exist_ok=True)
    yield


router = APIRouter(prefix="/api/v1/chat/back-music", tags=["Background Music"], lifespan=lifespan)


@router.get("/music_file/{music_file}")
async def get_specific_avatar(music_file: str):
    file_path = MUSIC_DIR / music_file
    if not file_path.exists():
        raise HTTPException(status_code=404, detail="没有找到音乐文件")

    return FileResponse(file_path)


@router.get("/list", response_model=List[Dict[str, str]])
async def get_music_list():
    """
    获取所有可用音乐列表
    返回:
        获取音乐的url地址和文件名
    """
    try:
        if not MUSIC_DIR.exists():
            MUSIC_DIR.mkdir(parents=True, exist_ok=True)
            return []

        music_files = []
        for file in MUSIC_DIR.iterdir():
            if file.is_file() and file.suffix.lower() in ALLOWED_EXTENSIONS:
                music_files.append({
                    "name": file.stem,
                    "url": file.name,
                })

        return music_files
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"无法获取文件: {str(e)}")


@router.post("/upload")
async def upload_music(file: UploadFile, name: str = None):
    """
    上传一个音乐文件到服务器
    """
    try:
        file_ext = Path(file.filename).suffix.lower()
        if file_ext not in ALLOWED_EXTENSIONS:
            raise HTTPException(status_code=400, detail="无效文件类型")

        MUSIC_DIR.mkdir(parents=True, exist_ok=True)

        filename = name if name else file.filename
        save_path = MUSIC_DIR / filename

        with save_path.open("wb") as buffer:
            shutil.copyfileobj(file.file, buffer)

        return JSONResponse(
            status_code=200,
            content={"message": "音乐上传成功"}
        )
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"未能上传音乐: {str(e)}")


@router.delete("/delete")
async def delete_music(url: str):
    """
    从服务器删除一个音乐文件
    """
    try:
        filename = url.split("/")[-1]
        file_path = MUSIC_DIR / filename

        logger.debug("删除音乐的寻找的路径是" + str(file_path))

        if not file_path.exists() or not file_path.is_file():
            raise HTTPException(status_code=404, detail="文件未找到")

        file_path.unlink()

        return JSONResponse(
            status_code=200,
            content={"message": "音乐成功删除了"}
        )
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"删除音乐的时候出现了问题: {str(e)}")
