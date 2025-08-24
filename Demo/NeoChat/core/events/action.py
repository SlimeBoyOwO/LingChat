# core/events/action.py
from typing import Dict, Any
from .base import BaseEventHandler
from ..logger import log_error

class ActionEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: Dict) -> bool:
        tool = params.get('Tool')
        var_name = params.get('Variable')
    
        if tool == 'Set':
            self.state.set_variable(var_name, content.get('Value'))
        elif tool == 'Calculate':
            self.state.calculate_variable(var_name, content.get('Expression'))
        elif tool == 'Random':
            self.state.set_random_variable(var_name, params.get('Min'), params.get('Max'))
        elif tool == 'RandomChoice':
            self.state.set_random_choice_variable(var_name, content.get('Choices'))
        else:
            log_error(f"未知的 Action Tool: {tool}")
        return True