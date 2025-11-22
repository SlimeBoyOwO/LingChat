from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

@dataclass
class AIServiceConfig:
    clients: set[str]
    user_id: str