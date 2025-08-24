# neochat/game/events/player.py
from typing import Dict
from .base import BaseEventHandler # 相对导入 BaseEventHandler

class PlayerEventHandler(BaseEventHandler):
    """
    处理玩家相关的事件，例如等待玩家输入或显示玩家预设对话。
    """
    def handle(self, params: Dict, content: str) -> bool:
        if params.get('Mode') == 'Input':
            # 进入等待玩家输入状态，并设置输入提示
            self.state.progress.runtime_state = 'WaitingForPlayerInput'
            self.state.progress.context['prompt'] = content
            return False  # 返回False，表示游戏循环需要暂停，等待玩家输入
        
        elif params.get('Mode') == 'Preset':
            # 显示玩家预设对话
            self.ui.display_player_dialogue(self.state.session.player.name, content)
            self.state.add_dialogue_history('Player', content=content)
            return True # 预设对话不阻塞游戏流程
        return True # 其他情况，游戏继续