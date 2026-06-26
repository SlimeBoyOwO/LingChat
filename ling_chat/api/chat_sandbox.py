"""
沙盒文件管理 API
提供安全的文件读写和命令执行接口
"""

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from ling_chat.core.agent_tools.sandbox import (
    sandbox_delete_file,
    sandbox_execute_command,
    sandbox_install_package,
    sandbox_list_files,
    sandbox_read_file,
    sandbox_write_file,
)
from ling_chat.core.logger import logger

router = APIRouter(prefix="/api/v1/chat/sandbox", tags=["Sandbox"])


class WriteFileRequest(BaseModel):
    path: str
    content: str


class DeleteFileRequest(BaseModel):
    path: str
    recursive: bool = False


class ExecuteCommandRequest(BaseModel):
    command: str
    timeout: int = 30


class InstallPackageRequest(BaseModel):
    package: str


@router.get("/read")
async def read_file(path: str = "."):
    """读取沙盒内文件"""
    result = sandbox_read_file(path)
    if not result.get("ok"):
        raise HTTPException(status_code=400, detail=result.get("error"))
    return result


@router.post("/write")
async def write_file(payload: WriteFileRequest):
    """写入沙盒内文件"""
    result = sandbox_write_file(payload.path, payload.content)
    if not result.get("ok"):
        raise HTTPException(status_code=400, detail=result.get("error"))
    return result


@router.get("/list")
async def list_files(path: str = "."):
    """列出沙盒内文件"""
    result = sandbox_list_files(path)
    if not result.get("ok"):
        raise HTTPException(status_code=400, detail=result.get("error"))
    return result


@router.post("/delete")
async def delete_file(payload: DeleteFileRequest):
    """删除沙盒内文件"""
    result = sandbox_delete_file(payload.path, payload.recursive)
    if not result.get("ok"):
        raise HTTPException(status_code=400, detail=result.get("error"))
    return result


@router.post("/execute")
async def execute_command(payload: ExecuteCommandRequest):
    """在沙盒内执行命令（需要白名单授权）"""
    logger.info(f"Sandbox execute: {payload.command}")
    result = sandbox_execute_command(payload.command, payload.timeout)
    return result


@router.post("/install")
async def install_package(payload: InstallPackageRequest):
    """安装 Python 包到沙盒目录"""
    logger.info(f"Sandbox install package: {payload.package}")
    result = sandbox_install_package(payload.package)
    return result
