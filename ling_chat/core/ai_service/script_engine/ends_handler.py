from ling_chat.core.logger import logger

class EndsHandler:
    def __init__(self):
        self.end_action:dict = {}

    def import_end(self, action: dict):
        self.end_action = action
        print("读取到的结束是" + str(action))
    
    def process_end(self) -> str:
        """根据章节的结束行为，返回下一章内容，如果没有则返回END"""
        if self.end_action.get("Type","") == "Linear":
            return self.end_action.get("NextCharpter","END")
        else:
            return "END"