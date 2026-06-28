import shutil
from pathlib import Path
from typing import Dict, List

from fastapi import APIRouter, HTTPException, UploadFile
from fastapi.responses import FileResponse, JSONResponse

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path

TEMPLATE_AMBIENT_DIR = static_path / "game_data/ambients"
AMBIENT_DIR = user_data_path / "game_data/ambients"
ALLOWED_EXTENSIONS = {".mp3", ".wav", ".flac", ".webm", ".weba", ".ogg", ".m4a", ".oga"}


def _ensure_ambient_dir() -> None:
    """确保环境音目录存在，首次使用时从模板目录初始化。"""
    if not AMBIENT_DIR.exists():
        if TEMPLATE_AMBIENT_DIR.exists():
            shutil.copytree(TEMPLATE_AMBIENT_DIR, AMBIENT_DIR, dirs_exist_ok=True)
        else:
            AMBIENT_DIR.mkdir(parents=True, exist_ok=True)


router = APIRouter(prefix="/api/v1/chat/ambient", tags=["Ambient Sound"])


@router.get("/list", response_model=List[Dict[str, str]])
async def get_ambient_list():
    """获取所有可用环境音文件列表"""
    try:
        _ensure_ambient_dir()
        if not AMBIENT_DIR.exists():
            return []

        ambient_files = []
        for file in sorted(
            AMBIENT_DIR.iterdir(), key=lambda f: f.stat().st_mtime, reverse=True
        ):
            if file.is_file() and file.suffix.lower() in ALLOWED_EXTENSIONS:
                ambient_files.append(
                    {
                        "name": file.stem,
                        "url": file.name,
                    }
                )

        return ambient_files
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"无法获取文件: {str(e)}") from e


@router.get("/ambient_file/{filename}")
async def get_ambient_file(filename: str):
    """提供环境音文件下载"""
    _ensure_ambient_dir()
    file_path = AMBIENT_DIR / filename
    if not file_path.exists():
        raise HTTPException(status_code=404, detail="没有找到环境音文件")

    return FileResponse(file_path)


@router.post("/upload")
async def upload_ambient(file: UploadFile, name: str | None = None):
    """上传一个环境音文件到服务器"""
    try:
        file_ext = Path(file.filename or "").suffix.lower()
        if file_ext not in ALLOWED_EXTENSIONS:
            raise HTTPException(status_code=400, detail="无效文件类型")

        _ensure_ambient_dir()

        filename = (name if name else file.filename) or ""
        if not filename:
            raise HTTPException(status_code=400, detail="无效文件名")
        save_path = AMBIENT_DIR / filename

        with save_path.open("wb") as buffer:
            shutil.copyfileobj(file.file, buffer)

        return JSONResponse(status_code=200, content={"message": "环境音上传成功"})
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"未能上传环境音: {str(e)}") from e


@router.delete("/delete")
async def delete_ambient(url: str):
    """从服务器删除一个环境音文件"""
    try:
        filename = url.split("/")[-1]
        file_path = AMBIENT_DIR / filename

        logger.debug("删除环境音的寻找路径是" + str(file_path))

        if not file_path.exists() or not file_path.is_file():
            raise HTTPException(status_code=404, detail="文件未找到")

        file_path.unlink()

        return JSONResponse(status_code=200, content={"message": "环境音已成功删除"})
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(
            status_code=500, detail=f"删除环境音时出现问题: {str(e)}"
        ) from e
