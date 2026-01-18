import os
from pathlib import Path

from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import temp_path

router = APIRouter(prefix="/api/v1/chat/sound", tags=["Chat Sound"])

@router.get("/get_voice/{voice_file}")
async def get_specific_sound(voice_file: str):

    voice_dir = Path(os.environ.get("TEMP_VOICE_DIR", temp_path / "audio"))

    file_path = voice_dir / voice_file

    logger.debug("语音寻找的路径是" + str(file_path))
    if not file_path.exists():
        raise HTTPException(status_code=404, detail="Voice not found")

    return FileResponse(file_path)
