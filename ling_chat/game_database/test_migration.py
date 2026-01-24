
import os
import sys
from pathlib import Path
from sqlmodel import Session, select

# Add project root to sys.path
project_root = Path(__file__).resolve().parent.parent.parent
sys.path.append(str(project_root))

from ling_chat.game_database.database import init_db, engine
from ling_chat.game_database.models import Role, UserInfo, Save, Line
from ling_chat.game_database.managers.role_manager import RoleManager
from ling_chat.game_database.managers.user_manager import UserManager
from ling_chat.game_database.managers.save_manager import SaveManager
from ling_chat.game_database.managers.memory_manager import MemoryManager

def test_managers():
    print("Initializing Database...")
    init_db()

    # 1. Test UserManager
    print("\n--- Testing UserManager ---")
    try:
        user = UserManager.create_user("test_user_v3", "password123")
        print(f"Created user: {user.username} (ID: {user.id})")
    except ValueError as e:
        print(f"User creation failed (expected if exists): {e}")
        user = UserManager.get_user_by_username("test_user_v3")
    
    assert user is not None
    
    # 2. Test RoleManager
    print("\n--- Testing RoleManager ---")
    with Session(engine) as session:
        role = Role(name="TestRole", resource_folder="test/path")
        session.add(role)
        session.commit()
        session.refresh(role)
        role_id = role.id
    
    UserManager.update_last_character(user.id, role_id)

    # 3. Test SaveManager
    print("\n--- Testing SaveManager ---")
    save = SaveManager.create_save(user.id, "Test Save Story V3")
    print(f"Created save: {save.title} (ID: {save.id})")
    
    # Test Append Single Message
    line1 = SaveManager.append_message(save.id, "Hello Single", "user")
    print(f"Appended Single Line: {line1.content}")
    
    # Test Bulk Append (New Feature)
    print("Testing Bulk Append...")
    bulk_messages = [
        {
            "content": "Message 1",
            "attribute": "assistant",
            "role_id": role_id,
            "original_emotion": "happy",
            "action_content": "waves"
        },
        {
            "content": "Message 2",
            "attribute": "user",
            "predicted_emotion": "excited"
        }
    ]
    
    new_lines = SaveManager.append_messages(save.id, bulk_messages)
    print(f"Bulk appended {len(new_lines)} lines")
    assert len(new_lines) == 2
    assert new_lines[0].original_emotion == "happy"
    assert new_lines[0].action_content == "waves"
    assert new_lines[1].predicted_emotion == "excited"
    
    # Check history order
    history = SaveManager.get_line_list(save.id)
    print(f"History length: {len(history)}")
    assert len(history) == 3 # 1 single + 2 bulk
    assert history[0].content == "Hello Single"
    assert history[1].content == "Message 1"
    assert history[2].content == "Message 2"
    
    print("History verification successful")

    print("\nALL TESTS PASSED!")

if __name__ == "__main__":
    test_managers()
