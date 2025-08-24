# core/utils/history_processor.py

from typing import List, Dict, Optional
from ..state_manager import StateManager
from ..logger import log_warning

def format_history_for_llm(
    state_manager: StateManager, 
    history_limit: int, 
    perspective_char_id: Optional[str] = None
) -> List[Dict[str, str]]:
    """
    统一格式化对话历史记录，为LLM调用做准备。
    
    :param state_manager: 游戏状态管理器实例。
    :param history_limit: 要包含的最大历史记录条数。
    :param perspective_char_id: 可选，指定哪个AI角色的视角。
                                 该角色的发言将被标记为 "assistant"，其他所有角色和玩家的发言为 "user"。
                                 如果为 None，则所有AI发言都是 "assistant"，玩家是 "user"。
    :return: 一个符合 OpenAI/DeepSeek API 格式的消息列表。
    """
    messages = []
    session = state_manager.session
    history_records = state_manager.dialogue_history[-history_limit:]

    for record in history_records:
        record_content = record.get('content') or record.get('data', {}).get('content')
        if not record_content:
            continue

        record_type = record.get('type')
        
        if record_type == 'Dialogue':
            char_id = record.get('data', {}).get('character_id')
            role = "assistant" if char_id == perspective_char_id else "user"
            
            # 如果不是从特定角色视角，需要标明发言者
            if perspective_char_id is not None and char_id != perspective_char_id:
                character = session.characters.get(char_id)
                char_name = character.name if character else "某人"
                record_content = f"{char_name}: {record_content}"
            
            messages.append({"role": role, "content": record_content})
        
        elif record_type == 'Player':
            player_name = session.player.name or "玩家"
            # 从AI视角看，玩家的发言是 "user"
            record_content_with_prefix = f"{player_name}: {record_content}"
            messages.append({"role": "user", "content": record_content_with_prefix})
        
        elif record_type == 'Narration':
            # 旁白通常作为背景信息提供给 "user"
            messages.append({"role": "user", "content": f"（旁白：{record_content}）"})
            
        elif record_type == 'Notice':
            # 公告也作为背景信息
            messages.append({"role": "user", "content": f"（公告：{record_content}）"})
        else:
            log_warning(f"在格式化历史记录时，跳过未处理的类型: {record_type}")

    return messages