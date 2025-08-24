# core/events/player.py
from typing import Dict
from .base import BaseEventHandler

class PlayerEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: str) -> bool:
        if params.get('Mode') == 'Input':
            self.state.progress.runtime_state = 'WaitingForPlayerInput'
            self.state.progress.context['prompt'] = content
            return False  # 返回False，暂停游戏循环等待输入
        
        elif params.get('Mode') == 'Preset':
            self.ui.display_player_dialogue(self.state.session.player.name, content)
            self.state.add_dialogue_history('Player', content=content)
            return True
        return True