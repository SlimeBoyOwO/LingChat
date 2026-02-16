from typing import List, Optional, Dict
from pathlib import Path
from sqlmodel import Session, select
from ling_chat.utils.function import Function
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import Role, RoleType
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.core.logger import logger
from ling_chat.schemas.character_settings import CharacterSettings

class RoleManager:
    @staticmethod
    def sync_roles_from_folder(game_data_path: Path) -> List[int]:
        """
        从游戏数据目录同步角色信息
        1. 扫描 game_data_path/characters 下的文件夹
        2. 读取 settings.txt
        3. 更新或创建 Role 记录
        """
        characters_dir = game_data_path / 'characters'
        if not characters_dir.exists():
            return []

        created_role_ids = []
        
        with Session(engine, expire_on_commit=False) as session:
            # 获取现有角色，以 resource_folder 为 Key
            # 注意：这里假设 resource_folder 是唯一的，对于本地资源文件夹来说通常如此
            statement = select(Role)
            results = session.exec(statement).all()
            db_roles_map = {role.resource_folder: role for role in results if role.resource_folder}
            
            # 遍历文件夹
            if characters_dir.exists():
                for entry in characters_dir.iterdir():
                    if not entry.is_dir() or entry.name == 'avatar':
                        continue
                    
                    settings_path_legacy = entry / 'settings.txt'
                    settings_path = entry / 'settings.yml'
                    if not settings_path.exists() and not settings_path_legacy.exists():
                        continue
                    
                    try:
                        resource_path_str = entry.name
                        
                        settings = Function.load_character_settings(entry)
                        title = settings.title or entry.name
                        
                        # 尝试从 settings 获取 script_key，或者默认 None
                        # 如果是主要角色，通常 script_key 可能为空，或者在 settings 里定义
                        # script_key_val = settings.get('script_key', None) 
                        
                        existing_role = db_roles_map.get(resource_path_str)
                        
                        if not existing_role:
                            # 创建新角色: 默认为 MAIN 类型，因为是从 characters 文件夹加载的
                            new_role = Role(
                                name=title, 
                                resource_folder=resource_path_str,
                                role_type=RoleType.MAIN, # characters 文件夹下的通常是主角
                            )
                            session.add(new_role)
                            session.commit()
                            session.refresh(new_role)
                            created_role_ids.append(new_role.id)
                        else:
                            # Update existing
                            changed = False
                            if existing_role.name != title:
                                existing_role.name = title
                                changed = True
                            
                            # 如果 settings 里有 script_key，也更新一下
                            # if script_key_val and existing_role.script_key != script_key_val:
                            #     existing_role.script_key = script_key_val
                            #     changed = True
                            
                            if changed:
                                session.add(existing_role)
                                session.commit()
                                
                    except Exception as e:
                        print(f"Error processing role {entry.name}: {e}")
                        continue
            
            session.commit()
            
        return created_role_ids
    
    @staticmethod
    def create_role(title: str, type: RoleType, resource_folder: str, script_key: Optional[str] = None, script_role_key: Optional[str] = None) -> Role:
        with Session(engine) as session:
            new_role = Role(name=title,
                            script_key=script_key,
                            script_role_key=script_role_key,
                            role_type=type,
                            resource_folder=resource_folder
                            )
            session.add(new_role)
            session.commit()
            return new_role

    @staticmethod
    def get_all_roles() -> List[Role]:
        with Session(engine) as session:
            return session.exec(select(Role)).all() # type: ignore
    
    @staticmethod
    def get_all_main_roles() -> List[Role]:
        with Session(engine) as session:
            return session.exec(select(Role).where(Role.role_type == RoleType.MAIN)).all() # type: ignore

    @staticmethod
    def get_role_by_id(role_id: int) -> Optional[Role]:
        with Session(engine) as session:
            return session.get(Role, role_id)
    
    @staticmethod
    def get_role_by_script_keys(script_key: str, script_role_key: str) -> Optional[Role]:
        """通过 script_key 查找角色 (可能返回多个中的第一个)"""
        with Session(engine) as session:
            stmt = select(Role).where(
                Role.script_key == script_key,
                Role.script_role_key == script_role_key
            )
            return session.exec(stmt).first()
    
    @staticmethod
    def get_script_roles(script_key: str) -> List[Role]:
        """获取剧本下的所有角色"""
        with Session(engine) as session:
            stmt = select(Role).where(Role.script_key == script_key)
            return list(session.exec(stmt).all())
        
    @staticmethod
    def get_role_settings_by_id(role_id: int) -> Optional[CharacterSettings]:
        role = RoleManager.get_role_by_id(role_id)
        if role is None or not role.resource_folder:
            return None
        
        path = Path()
        if role.role_type == RoleType.MAIN:
            path = user_data_path / "game_data" / "characters" / role.resource_folder
        elif role.role_type == RoleType.NPC:
            assert role.script_key is not None
            assert role.script_role_key is not None
            path  = user_data_path / "game_data" / "scripts" / role.script_key / "characters" / role.resource_folder

        if not (path / "settings.yml").exists() and not (path / "settings.txt").exists():
            logger.warning(f"角色设置文件不存在: {path}")
            return None

        settings = Function.load_character_settings(path)
        return settings
