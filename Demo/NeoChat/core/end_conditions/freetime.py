# core/end_conditions/freetime.py
from typing import Dict, Any
from .base import BaseEndConditionHandler
from ..logger import TermColors

class FreeTimeEndConditionHandler(BaseEndConditionHandler):
    def process(self, end_data: Dict[str, Any]):
        # 这个处理器同时处理 FreeTime 和 LimitedFreeTime
        self.state.progress.runtime_state = 'InFreeTime'
        self.state.progress.context['free_time_config'] = end_data
        self.state.progress.context['turns_taken'] = 0
        self.ui.display_system_message(
            end_data.get('InstructionToPlayer', '进入自由活动时间。'),
            color=TermColors.BLUE
        )