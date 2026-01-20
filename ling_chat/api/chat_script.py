import os

from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse

from ling_chat.core.service_manager import service_manager

router = APIRouter(prefix="/api/v1/chat/script", tags=["Chat Script"])

@router.get("/init_script/{script_name}")
async def init_script(script_name: str):
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=404, detail="AIService not found")
    scripts_manager = ai_service.scripts_manager

    result = {
        "script_name": script_name,
        "user_name": scripts_manager.game_context.player.user_name,
        "user_subtitle": scripts_manager.game_context.player.user_subtitle,
        "characters": {}
    }

    for settings in scripts_manager.get_script_characters(script_name):
        character_id = settings.get("character_id", "none")
        result["characters"][character_id] = {
            "ai_name": settings.get("ai_name", "无"),
            "ai_subtitle": settings.get("ai_subtitle", "无"),
            "thinking_message": ai_service.settings.get("thinking_message", "灵灵正在思考中..."),
            "scale": settings.get("scale", 1.0),
            "offset_x": settings.get("offset_x", 0),
            "offset_y": settings.get("offset_y", 0),
            "bubble_top": settings.get("bubble_top", 5),
            "bubble_left": settings.get("bubble_left", 20)
        }

    return result


@router.get("/get_script_avatar/{character}/{emotion}")
async def get_script_specific_avatar(character: str, emotion: str):
    ai_service = service_manager.ai_service

    if ai_service:
        file_path = ai_service.scripts_manager.get_avatar_dir(character) / (emotion + ".png")
    else:
        raise HTTPException(status_code=404, detail="AIService not found")

    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="Avatar not found")

    return FileResponse(file_path)

@router.get("/sound_file/{soundPath}")
async def get_script_sound(soundPath: str):
    try:
        ai_service = service_manager.ai_service
        if ai_service is None:
            raise HTTPException(status_code=404, detail="AISERVICE not found")
        else:
            file_path =  ai_service.scripts_manager.get_assests_dir() / "Sounds" / soundPath

        if not os.path.exists(file_path):
            raise HTTPException(status_code=404, detail="Sound not found")

        return FileResponse(file_path)
    except Exception as e:
        # 日志记录异常
        print(f"An error occurred: {e}")

@router.get("/music_file/{musicPath}")
async def get_script_music(musicPath: str):
    try:
        ai_service = service_manager.ai_service
        if ai_service is None:
            raise HTTPException(status_code=404, detail="AISERVICE not found")
        else:
            file_path =  ai_service.scripts_manager.get_assests_dir() / "Musics" / musicPath

        if not os.path.exists(file_path):
            raise HTTPException(status_code=404, detail="Music not found")

        return FileResponse(file_path)
    except Exception as e:
        # 日志记录异常
        print(f"An error occurred: {e}")

@router.get("/background_file/{background_file}")
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
