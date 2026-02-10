import os
from pathlib import Path

from fastapi import APIRouter, Body, HTTPException, Query
from fastapi.responses import FileResponse, JSONResponse
from typing import Dict, Any

from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.game_database.models import RoleType
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path

from ling_chat.game_database.managers.user_manager import UserManager
from ling_chat.game_database.managers.role_manager import RoleManager

router = APIRouter(prefix="/api/v1/chat/character", tags=["Chat Character"])


@router.post("/refresh_characters")
async def refresh_characters():
    try:
        RoleManager.sync_roles_from_folder(user_data_path / "game_data")
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

@router.post("/update_settings")
async def update_settings(
    role_id: int = Body(..., embed=True),
    settings: Dict[str, Any] = Body(..., embed=True)
):
    # 1. 获取角色信息以定位文件夹
    role = RoleManager.get_role_by_id(role_id)
    if not role:
        raise HTTPException(status_code=404, detail=f"角色{role_id}不存在")
    if not role.resource_folder:
        raise HTTPException(status_code=404, detail=f"角色{role_id}资源不存在")

    # 2. 定位路径
    base_path = Path()
    if role.role_type == RoleType.MAIN:
        base_path = user_data_path / "game_data" / "characters" / role.resource_folder
    elif role.role_type == RoleType.NPC:
        if not role.script_key:
            raise HTTPException(status_code=404, detail=f"角色{role_id}缺少剧本关联")
        base_path = user_data_path / "game_data" / "scripts" / role.script_key / "characters" / role.resource_folder
    else:
        raise HTTPException(status_code=404, detail=f"未知的角色类型")

    if not base_path.exists():
        raise HTTPException(status_code=404, detail="角色目录不存在")

    # 3. 保存设置
    try:
        # 确保传入的 settings 是完整的或者是合法的更新
        # 这里假设传入的是完整的 settings 字典
        success = Function.save_character_settings(base_path, settings)
        if success:
            return {"success": True, "message": "设置已保存"}
        else:
            raise HTTPException(status_code=500, detail="保存失败")
    except Exception as e:
        logger.error(f"保存角色设置失败: {str(e)}")
        raise HTTPException(status_code=500, detail=f"保存失败: {str(e)}")

@router.get("/get_role_info/{role_id}")
async def get_role_info(role_id: int):
    # 1. 获取角色信息
    role = RoleManager.get_role_by_id(role_id)
    if not role:
        raise HTTPException(status_code=404, detail=f"角色{role_id}不存在")
    if not role.resource_folder:
        raise HTTPException(status_code=404, detail=f"角色{role_id}资源不存在")
    
    settings = RoleManager.get_role_settings_by_id(role_id)

    if settings is None:
        raise HTTPException(status_code=404, detail=f"角色{role_id}设置不存在")
    return Function.convert_settings_to_role_info_dict(settings, role_id)
    

@router.get("/get_avatar/{role_id}/{emotion}/{clothes_name}")
async def get_role_avatar(
    role_id: int, 
    emotion: str, 
    clothes_name: str
):
    # 1. 获取角色信息
    role = RoleManager.get_role_by_id(role_id)
    if not role:
        raise HTTPException(status_code=404, detail=f"角色{role_id}不存在")
    if not role.resource_folder:
        raise HTTPException(status_code=404, detail=f"角色{role_id}资源不存在")
    
    # 2. 根据角色类型获取形象基础路径
    base_path = Path()
    if role.role_type == RoleType.MAIN:
        base_path = user_data_path / "game_data" / "characters" / Path(role.resource_folder) / "avatar"
    elif role.role_type == RoleType.NPC:
        # 确保 script_key 存在
        if not role.script_key:
             raise HTTPException(status_code=404, detail=f"角色{role_id}缺少剧本关联")
        base_path = user_data_path / "game_data" / "scripts" / Path(role.script_key) / "characters" / Path(role.resource_folder) / "avatar"
    else:
        # 处理可能的其他类型或默认情况
        raise HTTPException(status_code=404, detail=f"未知的角色类型")

    # 3. 处理服装路径
    # 如果 clothes_name 不是 default，则进入子文件夹
    avatar_path = base_path
    if clothes_name and clothes_name != 'default':
        avatar_path = base_path / clothes_name
    
    # 检查路径是否存在，防止 iterdir 报错
    if not avatar_path.exists():
        raise HTTPException(status_code=404, detail=f"角色头像目录不存在: {avatar_path}")

    # 4. 查找图片文件
    try:
        emotion_files = [
            f for f in avatar_path.iterdir() 
            if f.is_file() 
            and f.stem == emotion  # 关键修复：对比文件名(无后缀)是否等于传入的情绪名
            and f.suffix.lower() in [".png", ".jpg", ".jpeg"]
        ]
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"读取目录失败: {str(e)}")

    if not emotion_files:
        print(f"查找失败: Path={avatar_path}, Emotion={emotion}")
        raise HTTPException(status_code=404, detail=f"在路径 {avatar_path} 下未找到情绪 {emotion} 的图片")
    
    # 返回找到的第一个文件
    return FileResponse(emotion_files[0])


@router.post("/select_character")
async def select_character(
        user_id: int = Body(..., embed=True),
        character_id: int = Body(..., embed=True)
):
    try:
        # 1. 验证角色是否存在
        character = RoleManager.get_role_by_id(character_id)
        if not character:
            raise HTTPException(status_code=404, detail="角色不存在")

        # 2. 切换AI服务角色
        character_settings = RoleManager.get_role_settings_by_id(character_id)
        if character_settings is None:
            return HTTPException(status_code=500, detail="角色不存在")

        character_settings.character_id = character_id
        if service_manager.ai_service is not None:
            service_manager.ai_service.import_settings(settings=character_settings)
            service_manager.ai_service.reset_lines()

        # 2.5 更新用户的最后一次对话角色
        UserManager.update_last_character(user_id=user_id, role_id=character_id)

        # 3. 返回切换后的角色信息
        return {
            "success": True,
            "character": {
                "id": character.id,
                "title": character.name,
                "folder_name": character.resource_folder  # 用于前端加载角色专属提示
            }
        }
    except Exception as e:
        logger.error(f"切换角色失败: {str(e)}")
        raise HTTPException(status_code=500, detail="切换角色失败")


@router.get("/get_all_characters")
async def get_all_characters():
    try:
        db_chars = RoleManager.get_all_main_roles()

        if not db_chars:
            return {"data": [], "message": "未找到任何角色"}

        characters = []
        for char in db_chars:
            if not char.resource_folder:
                logger.warning(f"角色 {char.name} 缺少资源文件夹")
                continue

            char_path = user_data_path / "game_data" / "characters" / char.resource_folder

            settings = Function.load_character_settings(char_path)

            # 返回相对路径而不是完整路径
            avatar_relative_path = os.path.join(
                char.resource_folder,
                'avatar',
                '头像.png'
            )

            clothes_absolute_path = os.path.join(
                char_path,
                'avatar',
            )

            clothes_list = []
            for item in Path(clothes_absolute_path).iterdir():
                if item.is_dir():
                    clothes_list.append({
                        "title": item.name,
                        "avatar": str(item)
                    })

            characters.append({
                "character_id": char.id,
                "title": char.name,
                "info": settings.info or '这是一个人工智能对话助手',
                "avatar_path": avatar_relative_path,
                "clothes": clothes_list
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

@router.get("/clothes_file/{file_path:path}")
async def get_clothes_file(file_path: str):
    """获取服装相关文件"""
    if not os.path.exists(file_path):
        raise HTTPException(status_code=404, detail="文件不存在")

    return FileResponse(file_path)
