from typing import List, Optional, Dict
from pathlib import Path
from sqlmodel import Session, select
from ling_chat.utils.function import Function
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import Role
from ling_chat.utils.runtime_path import user_data_path

class RoleManager:
    @staticmethod
    def sync_roles_from_folder(game_data_path: Path) -> List[int]:
        """
        从游戏数据目录同步角色信息
        1. 扫描 game_data_path/characters 下的文件夹
        2. 读取 settings.txt
        3. 更新或创建 Role 记录
        4. 删除数据库中有但文件系统中不存在的角色
        """
        # 确保目录存在
        characters_dir = game_data_path / 'characters'
        if not characters_dir.exists():
            return []

        created_role_ids = []
        
        # 获取数据库中现有角色
        with Session(engine, expire_on_commit=False) as session:
            statement = select(Role)
            results = session.exec(statement).all()
            db_roles_map = {role.resource_folder: role for role in results}
            db_resource_paths = set(db_roles_map.keys())
            
            existing_resource_paths = set()
            
            # 遍历文件夹
            if characters_dir.exists():
                for entry in characters_dir.iterdir():
                    if not entry.is_dir() or entry.name == 'avatar':
                        continue
                    
                    settings_path = entry / 'settings.txt'
                    if not settings_path.exists():
                        continue
                    
                    try:
                        # 存储相对路径（相对于 game_data_path）
                        # 方法1：使用相对路径
                        # relative_path = entry.relative_to(game_data_path)
                        # resource_path_str = str(relative_path)  # 如 "characters/hero1"
                        
                        # 方法2：只存储角色文件夹名（如果角色文件夹名是唯一的）
                        resource_path_str = entry.name  # 如 "hero1"
                        
                        settings = Function.parse_enhanced_txt(str(settings_path))
                        title = settings.get('title', entry.name)
                        
                        existing_resource_paths.add(resource_path_str)
                        
                        existing_role = db_roles_map.get(resource_path_str)
                        
                        if not existing_role:
                            # 创建新角色
                            new_role = Role(name=title, resource_folder=resource_path_str)
                            session.add(new_role)
                            session.commit()
                            session.refresh(new_role)
                            created_role_ids.append(new_role.id)
                        else:
                            # 更新标题
                            if existing_role.name != title:
                                existing_role.name = title
                                session.add(existing_role)
                                session.commit()
                                
                    except Exception as e:
                        print(f"Error processing role {entry.name}: {e}")
                        continue
            
            # 删除多余角色 TODO：这里应该不删除，但提醒用户角色文件缺失，然后让用户选择补充资源或者删除角色
            # paths_to_delete = db_resource_paths - existing_resource_paths
            # for path in paths_to_delete:
            #     role_to_delete = db_roles_map[path]
            #     session.delete(role_to_delete)
            #     print(f"Deleted role: {role_to_delete.name}")
            
            session.commit()
            
        return created_role_ids

    @staticmethod
    def get_all_roles() -> List[Role]:
        with Session(engine) as session:
            return session.exec(select(Role)).all()

    @staticmethod
    def get_role_by_id(role_id: int) -> Optional[Role]:
        with Session(engine) as session:
            return session.get(Role, role_id)
        
    @staticmethod
    def get_role_settings_by_id(role_id: int) -> Optional[Dict]:
        role = RoleManager.get_role_by_id(role_id)
        if role is None:
            return None
        
        settings = Function.parse_enhanced_txt(user_data_path / "game_data" / "characters" / role.resource_folder / "settings.txt")
        return settings
