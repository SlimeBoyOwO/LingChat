from ling_chat.core.ai_service.type import GameMemoryBank, GameRole
from ling_chat.core.ai_service.game_system.role_manager import GameRoleManager


def test_memory_bank_roundtrip():
    mb = GameMemoryBank()
    dumped = mb.model_dump()
    loaded = GameMemoryBank.model_validate(dumped)
    assert loaded.model_dump() == dumped


def test_merge_memory_bank_keeps_single_system():
    mgr = GameRoleManager()

    memory = [
        {"role": "system", "content": "persona"},
        {"role": "user", "content": "hi"},
        {"role": "assistant", "content": "hello"},
    ]

    merged = mgr._merge_memory_bank_into_context(  # noqa: SLF001 (test private helper)
        memory,
        system_addendum="\n\n[MB] long term",
        short_term_prefix="[MB] short term\n",
    )

    # 仍然只有一个 system（合并到同一条）
    assert sum(1 for m in merged if m.get("role") == "system") == 1
    assert merged[0]["role"] == "system"
    assert "persona" in merged[0]["content"]
    assert "[MB] long term" in merged[0]["content"]

    # short term 被拼到 user 前缀
    user_msgs = [m for m in merged if m.get("role") == "user"]
    assert user_msgs
    assert user_msgs[0]["content"].startswith("[MB] short term")


def test_game_role_has_typed_memory_bank():
    role = GameRole(role_id=1)
    assert isinstance(role.memory_bank, GameMemoryBank)

