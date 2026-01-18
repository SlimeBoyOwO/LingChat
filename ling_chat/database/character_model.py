import os
import shutil
from pathlib import Path
from typing import Dict, List, Optional

from ling_chat.database.database import get_db_connection
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import static_path


class CharacterModel:
    @staticmethod
    def create_character(title: str, resource_path: str) -> Optional[int]:
        """创建新角色"""
        conn = get_db_connection()
        cursor = conn.cursor()

        try:
            # 检查是否已存在相同资源路径的角色
            cursor.execute("SELECT id FROM characters WHERE resource_path = ?", (resource_path,))
            if cursor.fetchone():
                raise ValueError(f"角色资源路径 '{resource_path}' 已存在")

            cursor.execute(
                "INSERT INTO characters (title, resource_path) VALUES (?, ?)",
                (title, resource_path)
            )
            character_id = cursor.lastrowid
            conn.commit()
            return character_id

        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()

    @staticmethod
    def get_character_by_id(character_id: int) -> Optional[Dict]:
        """根据ID获取角色信息"""
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM characters WHERE id = ?", (character_id,))
        row = cursor.fetchone()
        conn.close()
        return dict(row) if row else None

    @staticmethod
    def get_character_settings_by_id(character_id: int) -> Optional[Dict]:
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM characters WHERE id = ?", (character_id,))
        row = cursor.fetchone()
        conn.close()
        resource = dict(row)["resource_path"]
        settings = Function.parse_enhanced_txt(os.path.join(resource, "settings.txt"))
        return settings

    @staticmethod
    def get_all_characters() -> List[Dict]:
        """获取全部角色信息"""
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM characters")
        rows = cursor.fetchall()
        conn.close()
        return [dict(row) for row in rows] if rows else []

    @staticmethod
    def get_character_by_resource_path(resource_path: str) -> Optional[Dict]:
        """根据资源路径获取角色信息"""
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM characters WHERE resource_path = ?", (resource_path,))
        row = cursor.fetchone()
        conn.close()
        return dict(row) if row else None

    @staticmethod
    def update_character_title(character_id: int, title: str) -> None:
        """更新角色标题"""
        conn = get_db_connection()
        try:
            conn.execute(
                "UPDATE characters SET title = ? WHERE id = ?",
                (title, character_id)
            )
            conn.commit()
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()

    @staticmethod
    def delete_character(character_id: int) -> bool:
        """删除指定ID的角色

        Args:
            character_id: 要删除的角色ID

        Returns:
            bool: 是否成功删除

        Raises:
            ValueError: 如果角色ID无效
        """
        if not isinstance(character_id, int) or character_id <= 0:
            raise ValueError("无效的角色ID")

        conn = get_db_connection()
        cursor = conn.cursor()

        # 启用外键支持
        cursor.execute("PRAGMA foreign_keys = ON")

        try:
            # 先检查角色是否存在
            cursor.execute("SELECT id FROM characters WHERE id = ?", (character_id,))
            if not cursor.fetchone():
                conn.close()
                return False

            # 执行删除操作
            cursor.execute("DELETE FROM characters WHERE id = ?", (character_id,))
            conn.commit()
            return cursor.rowcount > 0
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()

    @staticmethod
    def sync_characters_from_game_data(game_data_path: Path) -> List[int]:
        """从游戏数据目录同步角色信息
        返回新创建的角色ID列表

        功能:
        1. 创建数据库中不存在的新角色
        2. 更新已有角色的标题
        3. 删除数据库中资源路径不存在的角色
        """
        new_character_ids = []
        characters_dir = str(game_data_path / 'characters')

        if not os.path.exists(characters_dir):
            game_data_template_path = static_path / 'game_data/characters'
            if Path(characters_dir).resolve() == game_data_template_path.resolve():
                raise ValueError(f"角色目录不存在: {characters_dir}")
            else:
                print(f"角色目录不存在: {characters_dir}, 创建默认角色目录")
                # copy game_data_template_path to characters_dir
                shutil.copytree(game_data_template_path, characters_dir, dirs_exist_ok=True)

        # 获取数据库中所有角色及其资源路径
        all_db_characters = CharacterModel.get_all_characters()
        db_characters_map = {char['resource_path']: char for char in all_db_characters}
        db_resource_paths = set(db_characters_map.keys())

        # 收集实际存在的角色资源路径
        existing_resource_paths = set()

        for character_name in os.listdir(characters_dir):
            character_path = os.path.join(characters_dir, character_name)
            if not os.path.isdir(character_path) or character_name == 'avatar':
                continue

            settings_path = os.path.join(character_path, 'settings.txt')
            if not os.path.exists(settings_path):
                continue

            try:
                settings = Function.parse_enhanced_txt(settings_path)
                resource_path = os.path.join(characters_dir, character_name)
                existing_resource_paths.add(resource_path)

                # 从settings中获取title，如果没有则使用角色名
                title = settings.get('title', character_name)

                # 检查角色是否已存在
                existing_char = db_characters_map.get(resource_path)
                if not existing_char:
                    # 创建新角色
                    char_id = CharacterModel.create_character(
                        title=title,
                        resource_path=resource_path
                    )
                    if char_id:
                        new_character_ids.append(char_id)
                else:
                    # 更新现有角色的标题
                    if existing_char['title'] != title:
                        CharacterModel.update_character_title(
                            existing_char['id'],
                            title
                        )
            except Exception as e:
                print(f"处理角色 {character_name} 时出错: {str(e)}")
                continue

        # 找出需要删除的角色(数据库中存在但文件系统中不存在的资源路径)
        paths_to_delete = db_resource_paths - existing_resource_paths
        for path in paths_to_delete:
            character = db_characters_map[path]
            try:
                CharacterModel.delete_character(character['id'])
                print(f"已删除不存在的角色: {character['title']} (ID: {character['id']})")
            except Exception as e:
                print(f"删除角色 {character['title']} 时出错: {str(e)}")

        return new_character_ids
