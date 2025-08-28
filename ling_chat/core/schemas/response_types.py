"""响应类型常量定义"""
class ResponseType:
    """响应类型常量"""
    # 普通对话响应
    AI_REPLY = "reply"
    
    # 剧本对话系统
    SCRIPT_DIALOG = "script_dialog"           # 剧本对话回复
    SCRIPT_PLAYER = "script_player"           # 剧本对话回复
    SCRIPT_NARRATION = "script_narration"   # 剧本旁白
    SCRIPT_CHOICE = "script_choice"         # 玩家选择分支
    
    # 场景管理系统
    SCRIPT_BACKGROUND = "script_background" # 背景切换
