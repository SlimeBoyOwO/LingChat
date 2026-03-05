from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent

class ChapterEndEvent(BaseEvent):
    """处理对话事件"""
    async def execute(self):
        end_type = self.event_data.get('end_type', 'linear')
        next_chapter = self.event_data.get('next_chapter', 'end')

        return next_chapter
    

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'chapter_end'