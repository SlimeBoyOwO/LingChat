import asyncio

from fastapi import APIRouter, Body, HTTPException
from typing import Optional

from ling_chat.core.adventure_trigger import adventure_trigger_system
from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.game_database.managers.adventure_manager import AdventureManager

router = APIRouter(prefix="/api/v1/chat/adventure", tags=["Chat Adventure"])


@router.get("/list/{character_folder}")
async def list_character_adventures(character_folder: str, user_id: int = 1):
    """获取指定角色的所有羁绊冒险列表（含解锁状态）"""
    try:
        ai_service = service_manager.get_ai_service()

        scripts_manager = ai_service.scripts_manager
        adventures = scripts_manager.get_character_adventures(character_folder)

        # 获取当前存档ID和剧本状态
        game_status = ai_service.game_status
        current_script_status = game_status.script_status

        result = []
        for adv_script in adventures:
            # 使用新的状态判断逻辑
            status = AdventureManager.get_adventure_status(
                user_id=user_id,
                game_status=game_status,
                adventure_folder=adv_script.folder_key,
                current_script_status=current_script_status,
            )

            # 获取解锁时间和完成时间
            unlocked_at = None
            completed_at = None
            if AdventureManager.is_unlocked(user_id, adv_script.folder_key):
                unlocks = AdventureManager.get_all_unlocked(user_id)
                for unlock in unlocks:
                    if unlock.adventure_folder == adv_script.folder_key:
                        unlocked_at = unlock.unlocked_at.isoformat() if unlock.unlocked_at else None
                        completed_at = unlock.completed_at.isoformat() if unlock.completed_at else None
                        break

            result.append({
                "adventure_folder": adv_script.folder_key,
                "name": adv_script.name,
                "description": adv_script.description,
                "recommand_start": adv_script.recommand_start,
                "order": adv_script.adventure.order,
                "status": status,
                "unlocked_at": unlocked_at,
                "completed_at": completed_at,
                "unlock_conditions": adv_script.adventure.unlock_conditions,
                "trigger": adv_script.adventure.trigger,
            })

        return {"data": result}
    except Exception as e:
        logger.error(f"获取角色冒险列表失败: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/all")
async def list_all_adventures(user_id: int = 1):
    """获取所有羁绊冒险（含解锁状态）"""
    try:
        ai_service = service_manager.ai_service
        if not ai_service:
            raise HTTPException(status_code=404, detail="AIService not found")

        scripts_manager = ai_service.scripts_manager
        all_adventures = scripts_manager.get_all_adventures()

        # 获取当前存档ID和剧本状态
        game_status = ai_service.game_status
        current_script_status = game_status.script_status

        result = []
        for adv_script in all_adventures:
            # 使用新的状态判断逻辑
            status = AdventureManager.get_adventure_status(
                user_id=user_id,
                game_status=game_status,
                adventure_folder=adv_script.folder_key,
                current_script_status=current_script_status,
            )

            result.append({
                "adventure_folder": adv_script.folder_key,
                "name": adv_script.name,
                "description": adv_script.description,
                "character_folder": adv_script.adventure.bound_character_folder,
                "order": adv_script.adventure.order,
                "status": status,
            })

        return {"data": result}
    except Exception as e:
        logger.error(f"获取全部冒险列表失败: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/progress/{user_id}")
async def get_user_progress(user_id: int):
    """获取用户的全部冒险解锁状态"""
    try:
        unlocks = AdventureManager.get_all_unlocked(user_id)
        return {
            "data": [
                {
                    "adventure_folder": u.adventure_folder,
                    "character_folder": u.character_folder,
                    "unlocked_at": u.unlocked_at.isoformat() if u.unlocked_at else None,
                }
                for u in unlocks
            ]
        }
    except Exception as e:
        logger.error(f"获取冒险进度失败: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/start")
async def start_adventure(
    user_id: int = Body(..., embed=True),
    adventure_folder: str = Body(..., embed=True),
):
    """启动指定羁绊冒险（触发剧本引擎）"""
    try:
        ai_service = service_manager.ai_service
        if not ai_service:
            raise HTTPException(status_code=404, detail="AIService not found")

        # 检查是否已解锁
        if not AdventureManager.is_unlocked(user_id, adventure_folder):
            raise HTTPException(status_code=403, detail="冒险尚未解锁")

        # 找到对应的剧本名称
        scripts_manager = ai_service.scripts_manager
        target_name = None
        for name, s in scripts_manager.all_scripts.items():
            if s.folder_key == adventure_folder:
                target_name = name
                break

        if not target_name:
            raise HTTPException(status_code=404, detail=f"找不到冒险剧本: {adventure_folder}")

        # 启动剧本
        asyncio.create_task(ai_service.start_script(target_name))

        return {"success": True, "message": f"冒险 {target_name} 已启动"}
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"启动冒险失败: {e}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/check_unlocks")
async def check_unlocks(user_id: int = Body(..., embed=True)):
    """手动检测是否有新冒险可解锁"""
    try:
        ai_service = service_manager.ai_service
        if not ai_service:
            raise HTTPException(status_code=404, detail="AIService not found")

        scripts_manager = ai_service.scripts_manager
        all_adventures = scripts_manager.get_all_adventures()
        chat_count = ai_service.game_status.get_chat_message_count()

        newly_unlocked = adventure_trigger_system.check_all_adventures(
            user_id=user_id,
            adventures=all_adventures,
            chat_count=chat_count,
            game_status=ai_service.game_status
        )

        return {"data": newly_unlocked, "count": len(newly_unlocked)}
    except Exception as e:
        logger.error(f"检测冒险解锁失败: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/complete")
async def complete_adventure(
    adventure_folder: str = Body(..., embed=True),
):
    """完成冒险（由剧本引擎结束时调用）"""
    try:
        ai_service = service_manager.ai_service
        if not ai_service:
            raise HTTPException(status_code=404, detail="AIService not found")

        # 获取当前存档ID
        save_id = ai_service.game_status.active_save_id
        if not save_id:
            raise HTTPException(status_code=400, detail="当前没有激活的存档")

        # 标记为已完成（存档级别）
        success = AdventureManager.mark_completed(save_id, adventure_folder)
        if not success:
            raise HTTPException(status_code=400, detail="标记完成失败")

        # 标记全局完成（用于解锁后续冒险）
        user_id = 1  # TODO: 从session或其他地方获取真实的user_id
        AdventureManager.mark_global_completed(user_id, adventure_folder)

        # 检查并触发完成成就
        scripts_manager = ai_service.scripts_manager
        unlocked_achievements = []
        for name, s in scripts_manager.all_scripts.items():
            if s.folder_key == adventure_folder and s.adventure.completion_achievements:
                from ling_chat.core.achievement_manager import achievement_manager
                for ach_def in s.adventure.completion_achievements:
                    ach_id = ach_def.get("id")
                    if ach_id:
                        result = achievement_manager.unlock(ach_id, ach_def)
                        if result:
                            unlocked_achievements.append(result)
                break

        # 检查是否有后续冒险被解锁
        # 注意：这里需要user_id，但我们没有从请求中获取
        # 暂时使用默认值1，后续可以改进
        user_id = 1  # TODO: 从session或其他地方获取真实的user_id
        all_adventures = scripts_manager.get_all_adventures()
        chat_count = ai_service.game_status.get_chat_message_count()
        newly_unlocked = adventure_trigger_system.check_all_adventures(
            user_id=user_id,
            adventures=all_adventures,
            chat_count=chat_count,
            game_status=ai_service.game_status,
        )

        return {
            "success": True,
            "unlocked_achievements": unlocked_achievements,
            "newly_unlocked_adventures": newly_unlocked,
        }
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"完成冒险失败: {e}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/reset")
async def reset_adventure(
    adventure_folder: str = Body(..., embed=True),
):
    """重置冒险进度以供重玩（从Save.status中移除completed标记）"""
    try:
        ai_service = service_manager.ai_service
        if not ai_service:
            raise HTTPException(status_code=404, detail="AIService not found")

        # 获取当前存档ID
        save_id = ai_service.game_status.active_save_id
        if not save_id:
            raise HTTPException(status_code=400, detail="当前没有激活的存档")

        # 从completed_scripts中移除
        from ling_chat.game_database.database import engine
        from ling_chat.game_database.models import Save
        from sqlmodel import Session

        with Session(engine) as session:
            save = session.get(Save, save_id)
            if not save:
                raise HTTPException(status_code=404, detail="存档不存在")

            completed_scripts = save.status.get("completed_scripts", [])
            if adventure_folder in completed_scripts:
                completed_scripts.remove(adventure_folder)
                save.status["completed_scripts"] = completed_scripts
                session.add(save)
                session.commit()
                logger.info(f"冒险 {adventure_folder} 已重置。")

        return {"success": True, "message": "冒险已重置"}
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"重置冒险失败: {e}")
        raise HTTPException(status_code=500, detail=str(e))
