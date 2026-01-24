import sys
import os
from sqlmodel import Session, select

# Add project root to sys.path
current_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(current_dir)
try:
    from ling_chat.game_database.database import engine, create_db_and_tables
    from ling_chat.game_database.models import Role, RoleType, UserInfo, Save, Line, LinePerception
    from ling_chat.core.ai_service.role_manager import RoleManager
except Exception as e:
    import traceback
    with open("verify_error.log", "w", encoding="utf-8") as f:
        traceback.print_exc(file=f)
        f.write(f"IMPORT ERROR: {e}\n")
    sys.exit(1)

def verify_perception():
    try:
        # 1. Init DB (Clean Slate)
        print("Resetting Database...")
        from sqlmodel import SQLModel
        SQLModel.metadata.drop_all(engine)
        create_db_and_tables()

        # 2. Setup Data
        with Session(engine) as session:
            print("Creating Roles...")
            # Create User
            user = UserInfo(username="tester", password="pw")
            session.add(user)
            session.commit()
            session.refresh(user)
            
            # Create Save
            save = Save(title="Test Save", user_id=user.id)
            session.add(save)
            session.commit()
            session.refresh(save)
            
            # Create Roles
            hero = Role(script_key="hero", name="Hero", role_type=RoleType.MAIN)
            guard = Role(script_key="guard", name="Guard", role_type=RoleType.NPC)
            narrator = Role(script_key="narrator", name="Narrator", role_type=RoleType.SYSTEM)
            session.add(hero)
            session.add(guard)
            session.add(narrator)
            session.commit()
            session.refresh(hero)
            session.refresh(guard)
            session.refresh(narrator)
            
            print(f"Roles Created: Hero({hero.id}), Guard({guard.id}), Narrator({narrator.id})")
            
            # Create Lines
            print("Creating Lines...")
            
            # L1: Hero speaks, Guard hears
            l1 = Line(
                content="Time to go.",
                attribute="assistant",
                sender_role_id=hero.id,
                save_id=save.id,
                display_name="Hero"
            )
            session.add(l1)
            session.commit()
            session.refresh(l1)
            
            # Add Perception for L1
            p1 = LinePerception(line_id=l1.id, role_id=guard.id)
            session.add(p1)
            
            # L2: Guard thinks (Self only), Hero does NOT hear
            l2 = Line(
                content="(I should verify identity...)",
                attribute="assistant",
                sender_role_id=guard.id,
                save_id=save.id,
                display_name="Guard"
            )
            session.add(l2)
            session.commit()
            session.refresh(l2)
            # No perception added -> Only Sender (Guard) knows
            
            # L3: Narrator speaks, Both hear
            l3 = Line(
                content="The wind blows.",
                attribute="system",
                sender_role_id=narrator.id,
                save_id=save.id,
                display_name="Narrator"
            )
            session.add(l3)
            session.commit()
            session.refresh(l3)
            
            session.add(LinePerception(line_id=l3.id, role_id=hero.id))
            session.add(LinePerception(line_id=l3.id, role_id=guard.id))
            
            session.commit()
            
            # Fetch lines with relationships loaded
            # Note: RoleManager expects lines with perception loaded
            # We need to query them carefully
            lines = session.exec(select(Line).where(Line.save_id == save.id)).all()
            # Ensure relations are loaded (accessing them triggers lazy load if session active)
            for l in lines:
                _ = l.perceived_by
                
            print(f"Total Lines: {len(lines)}")
            
            # 3. Test RoleManager
            print("Running RoleManager...")
            rm = RoleManager()
            rm.refresh_memories_from_lines(lines)
            
            # 4. Verify Hero Memory
            hero_mem = rm.get_history(role_id=hero.id)
            print(f"\nHero Memory ({len(hero_mem)} entries):")
            for m in hero_mem:
                print(f"  [{m['role']}] {m['content']}")
                
            # Expectation:
            # Hero said L1. -> Assistant: Time to go.
            # Hero perceived L3. -> System: The wind blows.
            # Hero NOT perceive L2.
            
            hero_content = "".join([m['content'] for m in hero_mem])
            assert "Time to go" in hero_content
            assert "The wind blows" in hero_content
            assert "verify identity" not in hero_content
            print(">>> Hero Memory Verification PASSED")
            
            # 5. Verify Guard Memory
            guard_mem = rm.get_history(role_id=guard.id)
            print(f"\nGuard Memory ({len(guard_mem)} entries):")
            for m in guard_mem:
                print(f"  [{m['role']}] {m['content']}")
                
            # Expectation:
            # Guard perceived L1. -> Context (User role block or processed as Other Assistant)
            # Guard said L2. -> Assistant: (verify identity)
            # Guard perceived L3. -> System: The wind blows.
            
            guard_content = "".join([m['content'] for m in guard_mem])
            assert "Time to go" in guard_content
            assert "The wind blows" in guard_content
            assert "verify identity" in guard_content # Guard's own thought
            print(">>> Guard Memory Verification PASSED")
            
            print(">>> Guard Memory Verification PASSED")
            
            # 6. Test GameStatus Integration
            print("Testing GameStatus Integration...")
            from ling_chat.core.ai_service.game_status import GameStatus
            from ling_chat.core.ai_service.type import GameRole
            from ling_chat.game_database.managers.save_manager import SaveManager
            from ling_chat.game_database.models import LineBase
            
            gs = GameStatus()
            
            # Setup Present Roles
            # Create GameRole instances mapping to DB IDs
            r_hero = GameRole(role_id=hero.id, display_name="Hero")
            r_guard = GameRole(role_id=guard.id, display_name="Guard")
            
            gs.present_roles = {r_hero, r_guard}
            
            # Create a new line via GameStatus
            # "Hero says: Let's move out." -> Should be perceived by Guard (and Hero)
            l_new = LineBase(
                content="Let's move out.",
                attribute="assistant",
                sender_role_id=hero.id,
                display_name="Hero"
            )
            
            gs.add_line(l_new)
            
            # Verify cached perception
            print(f"New Line Perceived IDs: {l_new.perceived_role_ids}")
            assert hero.id in l_new.perceived_role_ids
            assert guard.id in l_new.perceived_role_ids
            assert narrator.id not in l_new.perceived_role_ids
            
            # 7. Test Persistence via SaveManager
            print("Testing Persistence...")
            # Sync lines to DB
            SaveManager.sync_lines(save.id, [l_new])
            
            # Verify DB LinePerception
            lines_db = session.exec(select(Line).where(Line.content == "Let's move out.")).all()
            assert len(lines_db) > 0
            new_line_db = lines_db[0]
            
            # Check relationships
            # Note: access perceived_by to load it
            perceivers = {p.id for p in new_line_db.perceived_by}
            print(f"Persisted Perceivers: {perceivers}")
            
            assert hero.id in perceivers
            assert guard.id in perceivers
            
            print(">>> GameStatus & Persistence Verification PASSED")

    except Exception as e:
        import traceback
        with open("verify_runtime_error.log", "w", encoding="utf-8") as f:
            traceback.print_exc(file=f)
            f.write(f"RUNTIME ERROR: {e}\n")
        print(f"RUNTIME ERROR: {e}")
        sys.exit(1)

if __name__ == "__main__":
    try:
        verify_perception()
        print("\nALL TESTS PASSED")
    except Exception as e:
        print(f"\nTEST FAILED: {e}")
        import traceback
        traceback.print_exc()
