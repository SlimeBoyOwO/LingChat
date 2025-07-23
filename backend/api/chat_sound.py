import os
from pathlib import Path
from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse


router = APIRouter(prefix="/api/v1/chat/sound", tags=["Chat Sound"])

@router.get("/get_voice/{voice_file}")
async def get_specific_sound(voice_file: str):
    
    voice_dir = Path(os.environ.get("TEMP_VOICE_DIR", "data/voice"))

    file_path = os.path.join(voice_dir, voice_file)
    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="Voice not found")
    
    return FileResponse(file_path)
