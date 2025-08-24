# core/end_conditions/branching.py
from typing import Dict, Any
from .base import BaseEndConditionHandler
from neochat.platform.logging import log_error

class BranchingEndConditionHandler(BaseEndConditionHandler):
    def process(self, end_data: Dict[str, Any]):
        method = end_data.get('Method')
        if method == 'PlayerChoice':
            self.state.progress.runtime_state = 'WaitingForPlayerChoice'
            self.state.progress.context['choices_config'] = end_data.get('Branches', {})
        elif method == 'AIChoice':
            self.engine._execute_ai_choice(end_data)
        else:
            log_error(f"未知的 Branching Method: {method}")
            self.engine.game_over = True