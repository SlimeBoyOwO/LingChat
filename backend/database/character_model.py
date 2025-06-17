from typing import List, Dict, Optional
from .database import get_db_connection
import os
from utils.function import Function

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
        settings = Function.parse_enhanced_txt(os.path.join(resource,"settings.txt"))
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
    def sync_characters_from_game_data(game_data_path: str) -> List[int]:
        """从游戏数据目录同步角色信息
        返回新创建的角色ID列表
        """
        new_character_ids = []
        characters_dir = os.path.join(game_data_path, 'characters')
        
        if not os.path.exists(characters_dir):
            raise ValueError(f"角色目录不存在: {characters_dir}")

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
                
                # 从settings中获取title，如果没有则使用角色名
                title = settings.get('title', character_name)
                
                # 检查角色是否已存在
                existing_char = CharacterModel.get_character_by_resource_path(resource_path)
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
                    CharacterModel.update_character_title(
                        existing_char['id'],
                        title
                    )
            except Exception as e:
                print(f"处理角色 {character_name} 时出错: {str(e)}")
                continue

        return new_character_ids