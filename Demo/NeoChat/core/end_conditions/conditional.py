# core/end_conditions/conditional.py
from typing import Dict, Any
from .base import BaseEndConditionHandler

class ConditionalEndConditionHandler(BaseEndConditionHandler):
    def process(self, end_data: Dict[str, Any]):
        found_match = False
        for case in end_data.get('Cases', []):
            if self.state.evaluate_condition(case['Condition']):
                # 递归调用引擎的主处理函数，以支持嵌套的EndCondition
                self.engine._process_end_condition_recursively(case['Then'])
                found_match = True
                break
        if not found_match and 'Else' in end_data:
            self.engine._process_end_condition_recursively(end_data['Else'])