"""响应类型常量定义"""
class ResponseType:
    """响应类型常量"""
    # 普通对话响应
    AI_REPLY = "reply"
    
    # 剧本对话系统
    SCRIPT_DIALOG = "reply"           # 剧本对话回复
    SCRIPT_PLAYER = "player"           # 剧本对话回复
    SCRIPT_NARRATION = "narration"   # 剧本旁白
    SCRIPT_CHOICE = "choice"         # 玩家选择分支
    
    # 场景管理系统
    SCRIPT_BACKGROUND = "background" # 背景切换

    # 人物控制系统
    SCRIPT_MODIFY_CHARACTER = "modify_character" # 修改人物
