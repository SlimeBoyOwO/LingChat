class ScriptEngineError(Exception):
    """剧本引擎的基类异常"""
    pass

class ScriptLoadError(ScriptEngineError):
    """剧本加载失败时抛出"""
    pass

class ChapterLoadError(ScriptEngineError):
    """章节加载或解析失败时抛出"""
    pass
