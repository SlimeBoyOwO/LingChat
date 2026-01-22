from typing import Optional
from sqlmodel import Session, select
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import UserInfo, Role, Save

class UserManager:
    @staticmethod
    def create_user(username: str, password: str) -> UserInfo:
        with Session(engine, expire_on_commit=False) as session:
            # check exists
            statement = select(UserInfo).where(UserInfo.username == username)
            if session.exec(statement).first():
                raise ValueError(f"Username {username} already exists")
            
            user = UserInfo(username=username, password=password) # In real app, hash password
            session.add(user)
            session.commit()
            session.refresh(user)
            return user

    @staticmethod
    def get_user_by_id(user_id: int) -> Optional[UserInfo]:
        with Session(engine, expire_on_commit=False) as session:
            return session.get(UserInfo, user_id)
            
    @staticmethod
    def get_user_by_username(username: str) -> Optional[UserInfo]:
        with Session(engine, expire_on_commit=False) as session:
            statement = select(UserInfo).where(UserInfo.username == username)
            return session.exec(statement).first()

    @staticmethod
    def verify_password(username: str, password: str) -> Optional[UserInfo]:
        """Verify password (plaintext for now as per old logic)"""
        user = UserManager.get_user_by_username(username)
        if user and user.password == password:
            return user
        return None

    @staticmethod
    def update_last_character(user_id: int, role_id: int) -> bool:
        with Session(engine, expire_on_commit=False) as session:
            user = session.get(UserInfo, user_id)
            role = session.get(Role, role_id)
            if user and role:
                user.last_character_id = role.id
                session.add(user)
                session.commit()
                return True
            return False
        
    @staticmethod
    def update_last_save(user_id: int, save_id: int) -> bool:
        with Session(engine, expire_on_commit=False) as session:
            user = session.get(UserInfo, user_id)
            save = session.get(Save, save_id)
            if user and save:
                user.last_save_id = save.id
                session.add(user)
                session.commit()
                return True
            return False
