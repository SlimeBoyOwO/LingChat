
from ling_chat.core.ai_service.script_engine.type import Charpter
from ling_chat.core.logger import logger
from ling_chat.utils.function import Function

class CharpterManager:
    def __init__(self):
        self.charpter_name = "xxx"
        self.charpter:Charpter = Charpter("Charpter_01",[],{})

    def get_charpter(self, charpter_path: str) -> Charpter:
        config = Function.read_yaml_file(charpter_path)
        if config is not None:
            return Charpter(charpter_path, config.get('Events',[]), config.get('EndCondition',{}))
        else:
            return Charpter("ERROR",[],{})



