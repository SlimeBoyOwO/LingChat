import os
from pathlib import Path
from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse

from ling_chat.utils.runtime_path import temp_path

router = APIRouter(prefix="/api/v1/chat/sound", tags=["Chat Sound"])

@router.get("/get_voice/{voice_file}")
async def get_specific_sound(voice_file: str):
    
    voice_dir = temp_path / "data/voice"

    file_path = voice_dir / voice_file
    if not file_path.exists():
        raise HTTPException(status_code=404, detail="Voice not found")
    
    return FileResponse(file_path)
