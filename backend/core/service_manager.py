from core.ai_service.core import AIService
from .logger import logger
import os

class ServiceManager:
    _instance = None
    
    def __init__(self):
        self.ai_service = None
    
    def init_ai_service(self, settings):
        if self.ai_service is None:
            self.ai_service = AIService(settings)
        logger.info(f"🧠🧠🧠 ai_service 初始化")

        return self.ai_service

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

service_manager = ServiceManager.get_instance()