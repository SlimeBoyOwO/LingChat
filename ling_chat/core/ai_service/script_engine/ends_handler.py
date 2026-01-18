from ling_chat.core.ai_service.script_engine.type import GameContext


class EndsHandler:
    def __init__(self, end_action:dict, game_context: GameContext):
        self.game_context = game_context
        self.end_action:dict = end_action

    def process_end(self) -> str:
        """根据章节的结束行为，返回下一章内容，如果没有则返回end"""
        if self.end_action.get("type","") == "linear":
            return self.end_action.get("next","end")
        else:
            return "end"
