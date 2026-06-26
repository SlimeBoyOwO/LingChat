import asyncio
import json
import traceback
from pathlib import Path

from fastapi import APIRouter

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import user_data_path

_advanced_lock = asyncio.Lock()

# --- 工具函数 ---


def _get_json_path() -> Path:
    """获取高级功能数据文件路径"""
    game_data_path = user_data_path / "game_data"
    if not game_data_path.exists():
        game_data_path.mkdir(parents=True, exist_ok=True)
    return game_data_path / "advanced_features.json"


def _get_schedule_json_path() -> Path:
    """获取旧日程数据文件路径（用于迁移）"""
    game_data_path = user_data_path / "game_data"
    return game_data_path / "schedules.json"


def _load_data() -> dict:
    """读取 JSON 数据，如果不存在则从旧日程数据迁移"""
    json_path = _get_json_path()
    if not json_path.exists():
        # 尝试从旧 schedules.json 迁移 memoryNotes
        schedule_path = _get_schedule_json_path()
        if schedule_path.exists():
            try:
                with open(schedule_path, "r", encoding="utf-8") as f:
                    schedule_data = json.load(f)
                memory_notes = schedule_data.get("memoryNotes", [])
                # 写入新文件
                data = {"memoryNotes": memory_notes}
                _save_data(data)
                # 从旧文件中移除 memoryNotes（可选，保留兼容性）
                return data
            except Exception:
                pass
        return {"memoryNotes": []}
    try:
        with open(json_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError:
        logger.error(f"高级功能数据文件损坏，已重置为空数据: {json_path}")
        return {"memoryNotes": [], "_corrupted": True}
    except Exception as e:
        logger.error(f"读取高级功能数据文件失败: {json_path}: {e}", exc_info=True)
        return {"memoryNotes": [], "_corrupted": True}


def _save_data(data: dict):
    """写入 JSON 数据"""
    json_path = _get_json_path()
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)


router = APIRouter(prefix="/api/v1/chat/advanced", tags=["Chat Advanced Features"])


@router.get("/get")
async def get_advanced_features():
    """
    获取所有高级功能数据（记忆库等）
    """
    async with _advanced_lock:
        try:
            data = _load_data()
            corrupted = data.pop("_corrupted", False)
            result = {"code": 200, "msg": "success", "data": data}
            if corrupted:
                result["warn"] = "数据文件损坏，记忆库已重置为空"
                logger.warning("返回高级功能数据: 已损坏重置")
            return result
        except Exception as e:
            traceback.print_exc()
            return {"code": 500, "msg": str(e), "data": None}


@router.post("/save")
async def save_advanced_features(payload: dict):
    """
    保存高级功能数据。支持局部更新。
    """
    async with _advanced_lock:
        try:
            # 1. 读取现有数据
            current_data = _load_data()

            # 2. 更新数据 (只更新 payload 中不为 None 的字段)
            if "memoryNotes" in payload:
                current_data["memoryNotes"] = payload["memoryNotes"]

            # 3. 写入文件
            _save_data(current_data)

            return {"code": 200, "msg": "saved successfully"}

        except Exception as e:
            traceback.print_exc()
            return {"code": 500, "msg": str(e)}
