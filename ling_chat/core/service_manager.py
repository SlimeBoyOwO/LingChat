from ling_chat.core.ai_service.core import AIService
from ling_chat.database.user_model import UserModel
from ling_chat.database.character_model import CharacterModel
from ling_chat.utils.runtime_path import static_path, user_data_path
from ling_chat.utils.function import Function
from pathlib import Path
from ling_chat.core.logger import logger

class ServiceManager:
    _instance = None
    
    def __init__(self):
        self.ai_service = None

        self.user_services:dict[str, AIService] = {}  # user_id -> AIService
        self.client_mapping:dict[str, str] = {}  # client_id -> user_id
    
    def init_ai_service(self) -> AIService:
        self.ai_service = AIService(self.get_user_settings("1"))
        return self.ai_service
    
    def get_user_settings(self, user_id):
        user_id = "1" # TODO: 多用户之前的临时做法
        user_info = UserModel.get_user_by_id(user_id=user_id)
        if user_info is None:
            UserModel.create_user(username="admin", password="114514")
            user_info = UserModel.get_user_by_id(user_id=user_id)
        last_character_id = user_info.get("last_chat_character")
        character = CharacterModel.get_character_by_id(last_character_id)
        if character is not None and "resource_path" in character:
            resource_path = Path(character["resource_path"])
        else:
            resource_path = user_data_path / "game_data/characters/诺一钦灵"

        settings = Function.parse_enhanced_txt(str(resource_path / "settings.txt"))
        settings["character_id"] = last_character_id
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