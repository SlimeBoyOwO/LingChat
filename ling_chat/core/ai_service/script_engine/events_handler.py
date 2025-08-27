from ling_chat.core.logger import logger

class EventsHandler:
    def __init__(self):
        self.progress = 0
        self.event_list:list[dict] = []
        self.current_event:dict = {}
    
    def import_events(self, event_list:list[dict]):
        """导入事件"""
        self.event_list = event_list
        self.current_event = {}
        self.progress = 0

    def is_finished(self) -> bool:
        """判断所有事件是否处理完毕"""
        return self.progress >= len(self.event_list)
    
    async def process_next_event(self):
        """进行下一个事件的处理"""
        current_event = self.event_list[self.progress]
        self.progress += 1
        await self.process_event(current_event)
    
    async def process_event(self, event:dict):
        logger.info("事件" + str(self.progress) + "| " + str(event.keys()) + "| 拥有 |" + str(event.values()) + "") 
