# core/end_conditions/linear.py
from typing import Dict, Any
from .base import BaseEndConditionHandler

class LinearEndConditionHandler(BaseEndConditionHandler):
    def process(self, end_data: Dict[str, Any]):
        next_unit_id = end_data.get('NextUnitID')
        self.state.transition_to_unit(next_unit_id)