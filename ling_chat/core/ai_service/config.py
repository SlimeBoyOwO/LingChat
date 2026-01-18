from dataclasses import dataclass


@dataclass
class AIServiceConfig:
    clients: set[str]
    user_id: str
