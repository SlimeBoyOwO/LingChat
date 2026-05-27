"""LLM配置方案管理API路由

提供独立的LLM配置管理接口，与env_config.py解耦
"""

from typing import Any, Dict

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from ling_chat.configs.llm_config import llm_config
from ling_chat.configs.llm_toml_config import (
    find_key_in_toml,
    get_all_llm_toml_configs,
    parse_llm_toml,
)
from ling_chat.core.logger import logger

router = APIRouter(prefix="/api/v1/llm-config", tags=["LLM Config"])


class SwitchConfigRequest(BaseModel):
    """切换配置请求"""
    name: str = Field(..., description="配置方案名称")


class SaveConfigRequest(BaseModel):
    """保存配置请求"""
    name: str = Field(..., description="配置方案名称")
    config: Dict[str, Any] = Field(..., description="配置内容")


class DeleteConfigRequest(BaseModel):
    """删除配置请求"""
    name: str = Field(..., description="配置方案名称")


@router.get("/configs")
async def list_configs() -> Dict[str, Any]:
    """列出所有LLM配置方案"""
    try:
        configs = llm_config.list_configs()
        return {
            "status": "success",
            "data": configs,
        }
    except Exception as e:
        logger.error(f"列出LLM配置失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e


@router.get("/active")
async def get_active_config() -> Dict[str, Any]:
    """获取当前激活的配置详情"""
    try:
        return {
            "status": "success",
            "name": llm_config.get_active_config_name(),
            "config": llm_config.get_active_config(),
        }
    except Exception as e:
        logger.error(f"获取激活配置失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e


@router.post("/switch")
async def switch_config(request: SwitchConfigRequest) -> Dict[str, str]:
    """切换激活配置方案"""
    try:
        success = llm_config.set_active_config(request.name)
        if not success:
            raise HTTPException(
                status_code=404, detail=f"配置方案不存在: {request.name}"
            )
        return {
            "status": "success",
            "message": f"已切换到配置方案: {request.name}",
            "active": request.name,
        }
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"切换LLM配置失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e


@router.post("/save")
async def save_config(request: SaveConfigRequest) -> Dict[str, str]:
    """保存/更新配置方案"""
    try:
        llm_config.save_config(request.name, request.config)
        return {
            "status": "success",
            "message": f"已保存配置方案: {request.name}",
            "saved": request.name,
        }
    except Exception as e:
        logger.error(f"保存LLM配置失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e


@router.delete("/{name}")
async def delete_config(name: str) -> Dict[str, str]:
    """删除配置方案（default不可删除）"""
    try:
        llm_config.delete_config(name)
        return {
            "status": "success",
            "message": f"已删除配置方案: {name}",
            "deleted": name,
        }
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    except Exception as e:
        logger.error(f"删除LLM配置失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e


# ============================================================
# TOML 配置解析接口（与 env_config.py 的 parse_env_file 对齐）
# ============================================================

@router.get("/settings")
async def get_llm_toml_settings() -> Dict[str, Any]:
    """获取所有 LLM TOML 配置方案的结构化数据（与 /api/v1/config/settings 对齐）"""
    try:
        configs = get_all_llm_toml_configs()
        return configs
    except Exception as e:
        logger.error(f"获取LLM TOML配置失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e


@router.get("/key/{key}")
async def get_llm_toml_key(key: str) -> Dict[str, Any]:
    """在所有 TOML 配置中查找指定键（与 /api/v1/config/key/{key} 对齐）"""
    try:
        result = find_key_in_toml(key)
        if result is None:
            raise HTTPException(status_code=404, detail=f"Key '{key}' not found in any TOML config")
        return result
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"查找LLM TOML键失败: {e}")
        raise HTTPException(status_code=500, detail=str(e)) from e
