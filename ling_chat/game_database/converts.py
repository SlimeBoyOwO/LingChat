"""
模型转换器
集中管理各种模型之间的转换逻辑
"""
from typing import List, Optional
from sqlmodel import Session
from ling_chat.game_database.models import Line, GameLine

def line_to_game_line(line: Line) -> GameLine:
    """
    将数据库Line转换为运行时GameLine
    
    Args:
        line: 数据库Line对象
        
    Returns:
        GameLine: 运行时游戏行对象
    """
    if line is None:
        raise ValueError("Line对象不能为None")
    
    # 提取基础字段
    game_line_data = line.model_dump(
        exclude={'save_id', 'parent_line_id'}
    )
    
    # 提取感知的角色ID列表
    perceived_role_ids = []
    if hasattr(line, 'perceived_by') and line.perceived_by:
        perceived_role_ids = [role.id for role in line.perceived_by if role.id is not None]
    
    # 创建GameLine对象
    return GameLine(
        **game_line_data,
        perceived_role_ids=perceived_role_ids
    )


def lines_to_game_lines(lines: List[Line]) -> List[GameLine]:
    """
    批量转换Line列表为GameLine列表
    
    Args:
        lines: Line对象列表
        
    Returns:
        List[GameLine]: GameLine对象列表
    """
    return [line_to_game_line(line) for line in lines]
