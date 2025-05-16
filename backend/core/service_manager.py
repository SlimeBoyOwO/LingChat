from .ai_service import AIService

class ServiceManager:
    _instance = None
    
    def __init__(self):
        self.ai_service = AIService()
    
    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

service_manager = ServiceManager.get_instance()