from dataclasses import dataclass
from typing import Optional


@dataclass
class AIServiceConfig:
    clients: set[str]
    user_id: str
    last_active_client: Optional[str] = None
