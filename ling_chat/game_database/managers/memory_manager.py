from typing import List, Optional, Dict, Any
from sqlmodel import Session, select, delete
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import MemoryBank

class MemoryManager:
    """
    记忆仓库管理类
    处理 MemoryBank 表的增删改查
    """

    @staticmethod
    def add_memory(
        save_id: int, 
        info: Dict[str, Any], 
        role_id: Optional[int] = None, 
        script_role_id: Optional[str] = None
    ) -> MemoryBank:
        """
        添加一条新的记忆。
        :param save_id: 存档ID
        :param info: 记忆详情 (字典/JSON)
        :param role_id: 数据库角色ID (可选)
        :param script_role_id: 剧本角色ID (可选，用于未入库的临时角色)
        """
        with Session(engine, expire_on_commit=False) as session:
            memory = MemoryBank(
                save_id=save_id, 
                info=info, 
                role_id=role_id,
                script_role_id=script_role_id
            )
            session.add(memory)
            session.commit()
            session.refresh(memory)
            return memory

    @staticmethod
    def get_memories(
        save_id: int, 
        role_id: Optional[int] = None,
        script_role_id: Optional[str] = None
    ) -> List[MemoryBank]:
        """
        获取记忆列表。
        可以只传 save_id 获取该存档所有记忆，
        也可以指定 role_id 或 script_role_id 进行筛选。
        """
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(MemoryBank).where(MemoryBank.save_id == save_id)
            
            # 动态添加过滤条件
            if role_id is not None:
                stmt = stmt.where(MemoryBank.role_id == role_id)
            
            if script_role_id is not None:
                stmt = stmt.where(MemoryBank.script_role_id == script_role_id)
                
            return session.exec(stmt).all()

    @staticmethod
    def update_memory(
        memory_id: int, 
        new_info: Optional[Dict[str, Any]] = None,
        new_role_id: Optional[int] = None,
        new_script_role_id: Optional[str] = None
    ) -> Optional[MemoryBank]:
        """
        更新现有的记忆。
        仅传入需要修改的字段，传入 None 表示不修改该字段。
        """
        with Session(engine, expire_on_commit=False) as session:
            memory = session.get(MemoryBank, memory_id)
            if not memory:
                return None
            
            if new_info is not None:
                memory.info = new_info
            
            # 注意：这里使用 is not None 判断，允许将外键更新为其他值
            # 如果业务逻辑允许将角色ID清空，可以传入特殊值处理，这里仅处理非None更新
            if new_role_id is not None:
                memory.role_id = new_role_id
                
            if new_script_role_id is not None:
                memory.script_role_id = new_script_role_id

            session.add(memory)
            session.commit()
            session.refresh(memory)
            return memory

    @staticmethod
    def delete_memory(memory_id: int) -> bool:
        """
        根据主键 ID 删除单条记忆。
        """
        with Session(engine, expire_on_commit=False) as session:
            memory = session.get(MemoryBank, memory_id)
            if memory:
                session.delete(memory)
                session.commit()
                return True
            return False

    @staticmethod
    def delete_memories_by_role(
        save_id: int, 
        role_id: Optional[int] = None,
        script_role_id: Optional[str] = None
    ) -> int:
        """
        根据 存档ID 和 (角色ID 或 剧本角色ID) 批量删除记忆。
        常用于清空某个角色的记忆。
        :return: 被删除的行数
        """
        with Session(engine, expire_on_commit=False) as session:
            stmt = select(MemoryBank).where(MemoryBank.save_id == save_id)
            
            has_filter = False
            if role_id is not None:
                stmt = stmt.where(MemoryBank.role_id == role_id)
                has_filter = True
            
            if script_role_id is not None:
                stmt = stmt.where(MemoryBank.script_role_id == script_role_id)
                has_filter = True
            
            # 安全检查：如果没有指定任何角色过滤条件，是否允许删除该存档下的所有记忆？
            # 建议防止误删，要求至少有一个角色条件，或者调用方显式确认。
            # 这里如果不传角色ID，则不执行删除，防止清空整个存档记忆。
            if not has_filter:
                return 0

            memories = session.exec(stmt).all()
            count = len(memories)
            
            for memory in memories:
                session.delete(memory)
            
            session.commit()
            return count