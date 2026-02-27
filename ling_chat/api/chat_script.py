import os

from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse

from ling_chat.core.service_manager import service_manager

router = APIRouter(prefix="/api/v1/chat/script", tags=["Chat Script"])

@router.get("/list")
async def list_scripts():
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=404, detail="AIService not found")

    scripts_manager = ai_service.scripts_manager
    scripts = []
    for script_name in scripts_manager.get_script_list():
        script = scripts_manager.get_script(script_name)
        if script is None:
            continue
        scripts.append({
            "script_name": script.name,
            "description": script.description,
            "folder_key": script.folder_key,
            "intro_charpter": script.intro_chapter,
        })

    return scripts

@router.get("/init_script/{script_name}")
async def init_script(script_name: str):
    ai_service = service_manager.ai_service
    if not ai_service:
        raise HTTPException(status_code=404, detail="AIService not found")
    scripts_manager = ai_service.scripts_manager

    result = {
        "script_name": script_name,
        "user_name": scripts_manager.game_status.player.user_name,
        "user_subtitle": scripts_manager.game_status.player.user_subtitle,
        "characters": {}
    }

    for settings in scripts_manager.get_script_characters(script_name):
        character_id = settings.character_id
        result["characters"][character_id] = {
            "ai_name": settings.ai_name,
            "ai_subtitle": settings.ai_subtitle,
            "thinking_message": ai_service.settings.thinking_message or "灵灵正在思考中...",
            "scale": settings.scale,
            "offset_x": settings.offset_x,
            "offset_y": settings.offset_y,
            "bubble_top": settings.bubble_top,
            "bubble_left": settings.bubble_left,
            # 兼容扩展字段（若 settings.txt 未提供则前端可忽略）
            "clothes": settings.clothes,
            "clothes_name": settings.clothes_name,
            "body_part": settings.body_part,
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
            assets_dir = ai_service.scripts_manager.get_assests_dir()
            # 兼容旧/新目录命名
            candidates = [
                assets_dir / "Sounds" / soundPath,
                assets_dir / "SoundEffects" / soundPath,
            ]
            file_path = next((p for p in candidates if os.path.exists(p)), candidates[0])

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
            assets_dir = ai_service.scripts_manager.get_assests_dir()
            candidates = [
                assets_dir / "Musics" / musicPath,
                assets_dir / "BGMs" / musicPath,
            ]
            file_path = next((p for p in candidates if os.path.exists(p)), candidates[0])

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
            assets_dir = ai_service.scripts_manager.get_assests_dir()
            candidates = [
                assets_dir / "Backgrounds" / background_file,
                assets_dir / "Background" / background_file,
            ]
            file_path = next((p for p in candidates if os.path.exists(p)), candidates[0])

        if not os.path.exists(file_path):
            raise HTTPException(status_code=404, detail="Background not found")

        return FileResponse(file_path)
    except Exception as e:
        # 日志记录异常
        print(f"An error occurred: {e}")
