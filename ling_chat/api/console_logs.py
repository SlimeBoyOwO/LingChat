"""
前端控制台日志API
接收前端转发的控制台输出，并按照不同控制台输出等级分类为不同日志等级
"""
import traceback

from fastapi import APIRouter, Body, HTTPException, Request

from ling_chat.core.console_log_service import console_log_service
from ling_chat.core.logger import logger
from ling_chat.core.schemas.console_logs import (
    ConsoleLogBatch,
    ConsoleLogEntry,
    ConsoleLogLevel,
    ConsoleLogSource,
    LogFilterConfig,
    LogLevelMapping,
)

router = APIRouter(prefix="/api/v1/logs", tags=["Console Logs"])


@router.post("/console")
async def receive_console_log(
    request: Request,
    log_entry: ConsoleLogEntry = Body(..., embed=True)
):
    """
    接收单个前端控制台日志条目

    Args:
        log_entry: 前端控制台日志条目

    Returns:
        处理结果
    """
    try:
        logger.debug(f"收到前端控制台日志: level={log_entry.level}, message={log_entry.message[:100]}...")

        # 处理日志条目
        result = console_log_service.process_log_entry(log_entry)

        if result["processed"]:
            return {
                "code": 200,
                "data": {
                    "message": "日志处理成功",
                    "result": result
                }
            }
        else:
            return {
                "code": 200,
                "data": {
                    "message": "日志被过滤",
                    "result": result
                }
            }

    except Exception as e:
        logger.error(f"处理前端控制台日志失败: {str(e)}")
        traceback.print_exc()
        raise HTTPException(
            status_code=500,
            detail=f"处理日志失败: {str(e)}"
        )


@router.post("/console/batch")
async def receive_console_logs_batch(
    request: Request,
    log_batch: ConsoleLogBatch = Body(..., embed=True)
):
    """
    批量接收前端控制台日志条目

    Args:
        log_batch: 批量日志条目

    Returns:
        批量处理结果
    """
    try:
        logger.debug(f"收到批量前端控制台日志: count={len(log_batch.logs)}")

        results = []
        processed_count = 0
        filtered_count = 0
        error_count = 0

        for log_entry in log_batch.logs:
            try:
                result = console_log_service.process_log_entry(log_entry)
                results.append(result)

                if result["processed"]:
                    processed_count += 1
                else:
                    if result.get("reason") == "filtered":
                        filtered_count += 1
                    else:
                        error_count += 1
            except Exception as e:
                error_count += 1
                results.append({
                    "processed": False,
                    "error": str(e),
                    "log_entry": log_entry.model_dump()
                })

        return {
            "code": 200,
            "data": {
                "message": "批量日志处理完成",
                "summary": {
                    "total": len(log_batch.logs),
                    "processed": processed_count,
                    "filtered": filtered_count,
                    "errors": error_count
                },
                "batch_id": log_batch.batch_id,
                "session_id": log_batch.session_id,
                "user_id": log_batch.user_id,
                "results": results
            }
        }

    except Exception as e:
        logger.error(f"处理批量前端控制台日志失败: {str(e)}")
        traceback.print_exc()
        raise HTTPException(
            status_code=500,
            detail=f"处理批量日志失败: {str(e)}"
        )


@router.get("/levels")
async def get_log_levels():
    """
    获取可用的日志级别列表

    Returns:
        日志级别列表
    """
    try:
        levels = [level.value for level in ConsoleLogLevel]
        sources = [source.value for source in ConsoleLogSource]

        return {
            "code": 200,
            "data": {
                "console_levels": levels,
                "sources": sources,
                "level_mappings": console_log_service.get_stats()["level_mappings"]
            }
        }

    except Exception as e:
        logger.error(f"获取日志级别列表失败: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"获取日志级别列表失败: {str(e)}"
        )


@router.put("/mapping/{console_level}")
async def update_level_mapping(
    console_level: ConsoleLogLevel,
    mapping: LogLevelMapping = Body(..., embed=True)
):
    """
    更新日志级别映射配置

    Args:
        console_level: 前端控制台日志级别
        mapping: 新的映射配置

    Returns:
        更新结果
    """
    try:
        # 验证输入
        if mapping.console_log_level != console_level:
            raise HTTPException(
                status_code=400,
                detail="映射配置中的console_level必须与路径参数一致"
            )

        # 更新映射
        console_log_service.update_level_mapping(console_level, mapping)

        logger.info(f"更新日志级别映射: {console_level} -> {mapping.backend_log_level}")

        return {
            "code": 200,
            "data": {
                "message": "映射配置更新成功",
                "console_level": console_level.value,
                "mapping": mapping.model_dump()
            }
        }

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"更新日志级别映射失败: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"更新映射配置失败: {str(e)}"
        )


@router.put("/filters")
async def update_log_filters(
    filter_config: LogFilterConfig = Body(..., embed=True)
):
    """
    更新日志过滤配置

    Args:
        filter_config: 新的过滤配置

    Returns:
        更新结果
    """
    try:
        console_log_service.update_filter_config(filter_config)

        logger.info(f"更新日志过滤配置: min_level={filter_config.min_level}")

        return {
            "code": 200,
            "data": {
                "message": "过滤配置更新成功",
                "filter_config": filter_config.model_dump()
            }
        }

    except Exception as e:
        logger.error(f"更新日志过滤配置失败: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"更新过滤配置失败: {str(e)}"
        )


@router.get("/stats")
async def get_log_stats():
    """
    获取日志服务统计信息和环境变量配置

    Returns:
        服务统计信息和环境配置
    """
    try:
        stats = console_log_service.get_stats()

        # 获取环境变量配置
        import os
        env_config = {
            "ENABLE_FRONTEND_LOG_FORWARDING": os.environ.get('ENABLE_FRONTEND_LOG_FORWARDING', 'true'),
            "LOG_LEVEL": os.environ.get('LOG_LEVEL', 'INFO'),
            "ENABLE_FILE_LOGGING": os.environ.get('ENABLE_FILE_LOGGING', 'true'),
            "LOG_FILE_DIRECTORY": os.environ.get('LOG_FILE_DIRECTORY', 'ling_chat/data/run_logs')
        }

        return {
            "code": 200,
            "data": {
                "message": "获取统计信息成功",
                "stats": stats,
                "environment_config": env_config,
                "service_status": {
                    "enabled": os.environ.get('ENABLE_FRONTEND_LOG_FORWARDING', 'true').lower() == 'true',
                    "log_level": os.environ.get('LOG_LEVEL', 'INFO'),
                    "description": "前端日志转发服务状态（统一使用LOG_LEVEL过滤）"
                }
            }
        }

    except Exception as e:
        logger.error(f"获取日志服务统计信息失败: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"获取统计信息失败: {str(e)}"
        )


@router.post("/test")
async def test_log_processing(
    test_message: str = Body(..., embed=True),
    test_level: ConsoleLogLevel = Body(ConsoleLogLevel.INFO, embed=True)
):
    """
    测试日志处理功能

    Args:
        test_message: 测试消息
        test_level: 测试日志级别

    Returns:
        测试结果
    """
    try:
        # 创建测试日志条目
        test_entry = ConsoleLogEntry(
            level=test_level,
            message=test_message,
            source=ConsoleLogSource.FRONTEND,
            component="TestComponent",
            url="/test/url",
            line_number=42,
            column_number=10
        )

        # 处理测试日志
        result = console_log_service.process_log_entry(test_entry)

        return {
            "code": 200,
            "data": {
                "message": "测试完成",
                "test_entry": test_entry.model_dump(),
                "result": result
            }
        }

    except Exception as e:
        logger.error(f"测试日志处理失败: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"测试失败: {str(e)}"
        )
