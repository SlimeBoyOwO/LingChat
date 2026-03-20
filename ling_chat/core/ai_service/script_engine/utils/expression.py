"""
表达式求值工具（使用安全包装的 eval）
"""

import logging

logger = logging.getLogger(__name__)

def evaluate(expr: str, variables: dict) -> bool:
    """
    对表达式 expr 求值，返回布尔值。
    使用安全的 eval 包装，只允许访问 variables 中的变量。
    """
    if not expr:
        return True

    # 构建安全的全局和局部命名空间
    safe_globals = {
        "__builtins__": {},  # 禁用所有内置函数
        "True": True,
        "False": False,
        "None": None,
    }
    safe_locals = variables.copy()

    try:
        result = eval(expr, safe_globals, safe_locals)
        return bool(result)
    except NameError as e:
        logger.warning(f"表达式 '{expr}' 中的变量未定义: {e}")
        return False
    except SyntaxError as e:
        logger.error(f"表达式 '{expr}' 语法错误: {e}")
        return False
    except Exception as e:
        logger.error(f"表达式求值出错: {expr} - {e}")
        return False