from typing import List, Optional, Dict, Any
from sqlmodel import Session, select
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import MemoryBank

class MemoryManager:
    @staticmethod
    def add_memory(save_id: int, info: Dict[str, Any], role_id: Optional[int] = None) -> MemoryBank:
        with Session(engine, expire_on_commit=False) as session:
            memory = MemoryBank(save_id=save_id, info=info, role_id=role_id)
            session.add(memory)
            session.commit()
            session.refresh(memory)
            return memory

    @staticmethod
    def get_memories(save_id: int, role_id: Optional[int] = None) -> List[MemoryBank]:
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(MemoryBank).where(MemoryBank.save_id == save_id)
            if role_id:
                stmt = stmt.where(MemoryBank.role_id == role_id)
            return session.exec(stmt).all()

    @staticmethod
    def delete_memory(memory_id: int) -> bool:
        with Session(engine, expire_on_commit=False) as session:
            memory = session.get(MemoryBank, memory_id)
            if memory:
                session.delete(memory)
                session.commit()
                return True
            return False
