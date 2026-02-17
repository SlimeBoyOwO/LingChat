import traceback
import json
import os
from pathlib import Path

from fastapi import APIRouter

from ling_chat.utils.runtime_path import user_data_path
from ling_chat.schemas.schedule_settings import ScheduleDataPayload

# --- 工具函数 ---

def _get_json_path() -> Path:
    """获取数据文件路径"""
    game_data_path = user_data_path / "game_data"
    if not game_data_path.exists():
        game_data_path.mkdir(parents=True, exist_ok=True)
    return game_data_path / "schedules.json"

def _load_data() -> dict:
    """读取 JSON 数据，如果不存在则返回默认空结构"""
    json_path = _get_json_path()
    if not json_path.exists():
        return {
            "scheduleGroups": {},
            "todoGroups": {},
            "importantDays": []
        }
    try:
        with open(json_path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception:
        # 文件损坏等情况，返回空
        return {
            "scheduleGroups": {},
            "todoGroups": {},
            "importantDays": []
        }

def _save_data(data: dict):
    """写入 JSON 数据"""
    json_path = _get_json_path()
    with open(json_path, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

router = APIRouter(prefix="/api/v1/chat/schedule", tags=["Chat Schedule"])

@router.get("/get_schedules")
async def get_schedules():
    """
    获取所有日程、待办和日历数据
    """
    try:
        data = _load_data()
        return {
            "code": 200,
            "msg": "success",
            "data": data
        }
    except Exception as e:
        traceback.print_exc()
        return {"code": 500, "msg": str(e), "data": None}


@router.post("/save_schedules")
async def save_schedules(
    payload: ScheduleDataPayload,
):
    """
    保存数据。支持局部更新。
    前端只传 scheduleGroups 时，不会覆盖 todoGroups。
    """
    try:
        # 1. 读取现有数据
        current_data = _load_data()

        # 2. 更新数据 (只更新 payload 中不为 None 的字段)
        if payload.scheduleGroups is not None:
            current_data["scheduleGroups"] = payload.scheduleGroups
        
        if payload.todoGroups is not None:
            current_data["todoGroups"] = payload.todoGroups
            
        if payload.importantDays is not None:
            # Pydantic model 转 dict
            current_data["importantDays"] = [day.model_dump() for day in payload.importantDays]

        # 3. 写入文件
        _save_data(current_data)

        return {"code": 200, "msg": "saved successfully"}
    except Exception as e:
        import traceback
        traceback.print_exc()
        return {"code": 500, "msg": str(e)}
