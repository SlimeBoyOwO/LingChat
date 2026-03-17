from typing import List, Optional
from datetime import datetime
from sqlmodel import Session, select
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import AdventureUnlock, Save, RunningScript, AdventureStatus
from ling_chat.core.ai_service.type import ScriptStatus
from ling_chat.core.logger import logger


class AdventureManager:
    """羁绊冒险管理器 — 负责冒险解锁/完成/重置等持久化操作"""

    # ============ 解锁状态管理（全局，user_id级别）============

    @staticmethod
    def is_unlocked(user_id: int, adventure_folder: str) -> bool:
        """检查冒险是否已解锁（全局）"""
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(AdventureUnlock).where(
                AdventureUnlock.user_id == user_id,
                AdventureUnlock.adventure_folder == adventure_folder,
            )
            return session.exec(stmt).first() is not None

    @staticmethod
    def get_all_unlocked(user_id: int) -> List[AdventureUnlock]:
        """获取用户所有已解锁的冒险"""
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(AdventureUnlock).where(
                AdventureUnlock.user_id == user_id,
            )
            return list(session.exec(stmt).all())

    @staticmethod
    def unlock_adventure(user_id: int, adventure_folder: str, character_folder: str) -> AdventureUnlock:
        """解锁冒险（全局）"""
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(AdventureUnlock).where(
                AdventureUnlock.user_id == user_id,
                AdventureUnlock.adventure_folder == adventure_folder,
            )
            unlock = session.exec(stmt).first()

            if unlock:
                logger.info(f"冒险 {adventure_folder} 已经解锁，跳过。")
                return unlock

            unlock = AdventureUnlock(
                user_id=user_id,
                adventure_folder=adventure_folder,
                character_folder=character_folder,
                unlocked_at=datetime.now(),
            )
            session.add(unlock)
            session.commit()
            session.refresh(unlock)
            logger.info(f"冒险 {adventure_folder} 已解锁。")
            return unlock

    @staticmethod
    def is_global_completed(user_id: int, adventure_folder: str) -> bool:
        """检查冒险是否已全局完成（从 AdventureUnlock.completed_at 查询）"""
        if not adventure_folder:
            return True
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(AdventureUnlock).where(
                AdventureUnlock.user_id == user_id,
                AdventureUnlock.adventure_folder == adventure_folder,
            )
            unlock = session.exec(stmt).first()
            return unlock is not None and unlock.completed_at is not None

    @staticmethod
    def mark_global_completed(user_id: int, adventure_folder: str) -> bool:
        """标记冒险为全局已完成（更新 AdventureUnlock.completed_at）"""
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(AdventureUnlock).where(
                AdventureUnlock.user_id == user_id,
                AdventureUnlock.adventure_folder == adventure_folder,
            )
            unlock = session.exec(stmt).first()

            if not unlock:
                logger.error(f"冒险未解锁，无法标记完成: {adventure_folder}")
                return False

            if unlock.completed_at:
                logger.info(f"冒险 {adventure_folder} 已经全局完成，跳过。")
                return True

            unlock.completed_at = datetime.now()
            session.add(unlock)
            session.commit()
            logger.info(f"冒险 {adventure_folder} 已标记为全局完成。")
            return True

    # ============ 完成状态管理（存档级别，通过Save.status）============

    @staticmethod
    def is_completed(save_id: int, adventure_folder: str) -> bool:
        """检查冒险是否已完成（从Save.status.completed_scripts查询）"""
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                return False

            completed_scripts = save.status.get("completed_scripts", [])
            return adventure_folder in completed_scripts

    @staticmethod
    def mark_completed(save_id: int, adventure_folder: str) -> bool:
        """标记冒险为已完成（更新Save.status.completed_scripts）"""
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                logger.error(f"存档不存在: save_id={save_id}")
                return False

            # 获取已完成列表
            completed_scripts = save.status.get("completed_scripts", [])

            # 如果已经完成，跳过
            if adventure_folder in completed_scripts:
                logger.info(f"冒险 {adventure_folder} 已经完成，跳过。")
                return True

            # 添加到完成列表
            completed_scripts.append(adventure_folder)
            save.status["completed_scripts"] = completed_scripts

            # 标记字段已修改（SQLModel需要）
            session.add(save)
            session.commit()
            logger.info(f"冒险 {adventure_folder} 已标记为完成。")
            return True

    @staticmethod
    def get_completed_adventures(save_id: int) -> List[str]:
        """获取存档已完成的所有冒险（从Save.status.completed_scripts读取）"""
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                return []

            return save.status.get("completed_scripts", [])

    # ============ 综合状态查询 ============

    @staticmethod
    def get_adventure_status(
        user_id: int,
        game_status: GameStatus,
        adventure_folder: str,
        current_script_status: Optional[ScriptStatus] = None,
    ) -> str:
        """
        综合判断冒险状态：
        1. 检查是否解锁（AdventureUnlock表）
        2. 检查是否正在进行（script_status）
        3. 检查是否已完成（Save.status.completed_scripts）

        返回：LOCKED, UNLOCKED, IN_PROGRESS, COMPLETED
        """
        # 1. 检查是否解锁
        if not AdventureManager.is_unlocked(user_id, adventure_folder):
            return AdventureStatus.LOCKED

        # 2. 检查是否正在进行
        if current_script_status and current_script_status.folder_key == adventure_folder:
            return AdventureStatus.IN_PROGRESS

        # 3. 检查是否已完成
        if adventure_folder in game_status.completed_scripts:
            return AdventureStatus.COMPLETED

        # 4. 已解锁但未开始
        return AdventureStatus.UNLOCKED

