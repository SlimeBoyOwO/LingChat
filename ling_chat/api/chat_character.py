import os
from pathlib import Path
from fastapi import APIRouter, Body, HTTPException
from fastapi.responses import FileResponse, JSONResponse
from ling_chat.core.service_manager import service_manager
from ling_chat.database.character_model import CharacterModel
from ling_chat.database.user_model import UserModel
from ling_chat.utils.function import Function
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path

router = APIRouter(prefix="/api/v1/chat/character", tags=["Chat Character"])


@router.post("/refresh_characters")
async def refresh_characters():
    try:
        CharacterModel.sync_characters_from_game_data(user_data_path / "game_data")
        return {"success": True}
    except Exception as e:
        logger.error(f"刷新人物列表请求失败: {str(e)}")
        raise HTTPException(status_code=500, detail="刷新人物列表失败")


@router.get("/open_web")
async def open_creative_web():
    try:
        import webbrowser
        url = "https://github.com/SlimeBoyOwO/LingChat/discussions"
        webbrowser.open(url)
    except Exception as e:
        logger.error(f"无法使用浏览器启动创意工坊: {str(e)}")
        raise HTTPException(status_code=500, detail="无法使用浏览器启动网页")


@router.get("/get_avatar/{avatar_file}")
async def get_specific_avatar(avatar_file: str):
    ai_service = service_manager.ai_service

    if not ai_service or not ai_service.character_path:
        raise HTTPException(status_code=404, detail="AIService or character_path not found")

    file_path = Path(ai_service.character_path) / "avatar" / avatar_file
    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="Avatar not found")

    return FileResponse(file_path)

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


@router.post("/select_character")
async def select_character(
        user_id: int = Body(..., embed=True),
        character_id: int = Body(..., embed=True)
):
    try:
        # 1. 验证角色是否存在
        character = CharacterModel.get_character_by_id(character_id=character_id)
        if not character:
            raise HTTPException(status_code=404, detail="角色不存在")

        # 2. 切换AI服务角色
        character_settings = CharacterModel.get_character_settings_by_id(character_id=character_id)
        if character_settings is None: return HTTPException(status_code=500, detail="角色不存在")

        character_settings["character_id"] = character_id
        service_manager.ai_service.import_settings(settings=character_settings)
        service_manager.ai_service.reset_memory()

        # 2.5 更新用户的最后一次对话角色
        UserModel.update_user_character(
            user_id=user_id,
            character_id=character_id
        )

        # 3. 可以返回切换后的角色信息
        return {
            "success": True,
            "character": {
                "id": character.get("id"),
                "title": character.get("title")
            }
        }
    except Exception as e:
        logger.error(f"切换角色失败: {str(e)}")
        raise HTTPException(status_code=500, detail="切换角色失败")


@router.get("/get_all_characters")
async def get_all_characters():
    try:
        db_chars = CharacterModel.get_all_characters()

        if not db_chars:
            return {"data": [], "message": "未找到任何角色"}

        characters = []
        for char in db_chars:
            settings_path = os.path.join(char['resource_path'], 'settings.txt')
            settings = Function.parse_enhanced_txt(settings_path)

            # 返回相对路径而不是完整路径
            avatar_relative_path = os.path.join(
                os.path.basename(char['resource_path']),
                'avatar',
                '头像.png'
            )

            characters.append({
                "character_id": char['id'],
                "title": char['title'],
                "info": settings.get('info', '这是一个人工智能对话助手'),
                "avatar_path": avatar_relative_path  # 修改为相对路径
            })

        return {"data": characters}

    except FileNotFoundError as e:
        logger.error(f"角色配置文件缺失: {str(e)}")
        return JSONResponse(status_code=500, content={"message": "角色配置文件缺失"})
    except Exception as e:
        logger.error(f"获取角色列表失败: {str(e)}")
        return JSONResponse(status_code=500, content={"message": "获取角色列表失败"})


@router.get("/character_file/{file_path:path}")
async def get_character_file(file_path: str):
    """获取角色相关文件(头像等)"""
    full_path = user_data_path / f"game_data/characters/{file_path}"

    if not os.path.exists(full_path):
        raise HTTPException(status_code=404, detail="文件不存在")

    return FileResponse(full_path)