from pathlib import Path

from ling_chat.core.ai_service.core import AIService
from ling_chat.core.logger import logger

from ling_chat.game_database.managers.user_manager import UserManager
from ling_chat.game_database.managers.role_manager import RoleManager

from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path


class ServiceManager:
    _instance = None

    def __init__(self):
        self.ai_service = None

        self.user_services:dict[str, AIService] = {}  # user_id -> AIService
        self.client_mapping:dict[str, str] = {}  # client_id -> user_id

    def init_ai_service(self) -> AIService:
        self.ai_service = AIService(self.get_user_settings(1))
        return self.ai_service
    
    def get_user_settings(self, user_id: int):
        user_id = 1 # TODO: 多用户之前的临时做法
        user_info = UserManager.get_user_by_id(user_id=user_id)

        # 如果用户信息是空的，说明一个用户都没有，游戏进行初始化，创建一个管理员用户
        if user_info is None:
            UserManager.create_user(username="admin", password="114514")
            user_info = UserManager.get_user_by_id(user_id=user_id)
        
        if user_info is None:
            logger.error("游戏数据库初始化失败，未能创建管理员账户")
            return self.get_default_character()

        last_character_id = user_info.last_character_id

        # 如果用户没有选择过角色，则默认选择第一个角色
        if last_character_id is None:
            last_character_id = 1
            UserManager.update_last_character(user_id=user_id, role_id=last_character_id)

        character = RoleManager.get_role_by_id(last_character_id)

        character_dir = user_data_path / "game_data" / "characters"

        if character is not None:
            resource_path = character_dir / character.resource_folder
        else:
            resource_path = character_dir / "诺一钦灵"

        settings = Function.parse_enhanced_txt(str(resource_path / "settings.txt"))
        settings["character_id"] = last_character_id
        return settings
    
    def get_default_character(self):
        character_dir = user_data_path / "game_data" / "characters"
        resource_path = character_dir / "诺一钦灵"
        settings = Function.parse_enhanced_txt(str(resource_path / "settings.txt"))
        settings["character_id"] = 0
        return settings

    async def add_client(self, client_id):
        if self.ai_service is not None:
            await self.ai_service.add_client(client_id)

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

service_manager = ServiceManager.get_instance()
