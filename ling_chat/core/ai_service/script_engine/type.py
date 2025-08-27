from dataclasses import dataclass, field

@dataclass
class Script:
    name: str
    description: str
    intro_charpter: str

@dataclass
class Character:
    character_id: str
    settings: dict
    resource_path: str
    memory: list[dict]

@dataclass
class Charpter:
    charpter_id: str
    events: list[dict]
    ends: dict
