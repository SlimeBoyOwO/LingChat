import os
import threading

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel
from ling_chat.core.logger import logger

from ..update.update_main import create_application

router = APIRouter(prefix="/api/v1/update", tags=["Update"])

# 状态和配置
update_status = {
    "status": "idle",
    "progress": 0,
    "message": "",
    "update_info": None,
    "error": None
}

update_config = {
    "auto_backup": True,
    "auto_apply": False
}

# 请求模型
class UpdateConfig(BaseModel):
    auto_backup: bool = True
    auto_apply: bool = False

class ApplyUpdateRequest(BaseModel):
    backup: bool = True

# 初始化更新应用
def init_update_application():
    """初始化更新应用，使用项目根目录作为应用目录"""
    # 获取项目根目录
    # 从当前文件位置向上三级：ling_chat/api/update_api.py -> ling_chat/api -> ling_chat -> 项目根目录
    current_file = os.path.abspath(__file__)
    project_root = os.path.dirname(os.path.dirname(os.path.dirname(current_file)))

    # 获取更新URL
    update_base = os.getenv("UPDATE_URL", "http://localhost:5100").strip()
    if update_base.endswith("/updates"):
        update_url = update_base
    else:
        update_url = f"{update_base.rstrip('/')}/updates"

    logger.info(f"初始化更新应用，项目根目录: {project_root}")
    logger.info(f"更新URL: {update_url}")

    return create_application(
        version_file=os.path.join(project_root, "version"),
        update_url=update_url,
        app_dir=project_root  # 传递项目根目录
    )

# 初始化更新应用
update_application = init_update_application()

# 回调函数
def update_status_callback(status, old_status=None):
    update_status["status"] = status.value
    if status.value == "error":
        update_status["message"] = "更新过程中出现错误"

def update_progress_callback(progress):
    update_status["progress"] = progress
    if progress == 100:
        update_status["message"] = "操作完成"

def update_available_callback(update_info):
    update_status["update_info"] = update_info
    update_status["message"] = f"发现新版本: {update_info.get('version', '未知')}"

def update_completed_callback(update_info):
    update_status["message"] = "更新完成，请重启应用"
    update_status["status"] = "completed"

def error_callback(error):
    update_status["error"] = error
    update_status["status"] = "error"
    update_status["message"] = f"错误: {error}"

# 注册回调
update_application.update_manager.register_callback("status_changed", update_status_callback)
update_application.update_manager.register_callback("progress_changed", update_progress_callback)
update_application.update_manager.register_callback("update_available", update_available_callback)
update_application.update_manager.register_callback("update_completed", update_completed_callback)
update_application.update_manager.register_callback("error_occurred", error_callback)

# 线程锁，防止并发更新
update_lock = threading.Lock()

def execute_update_operation(operation_type, backup=True):
    """执行更新操作"""
    try:
        if operation_type == "check":
            found = update_application.manual_check_update()
            if found:
                update_info = update_application.update_manager.get_update_info()
                update_status["update_info"] = update_info
                update_status["message"] = "发现新版本"

                # 处理更新链信息
                if update_info and update_info.get('update_chain'):
                    update_chain = update_info.get('update_chain', [])
                    if update_chain:
                        update_status["message"] = f"发现 {len(update_chain)} 个待更新版本"
                else:
                    # 单版本兼容
                    target_version = update_info.get('target_version') if update_info else None
                    version_str = update_info.get('version') if update_info else None
                    display_version = target_version or version_str or '未知'
                    update_status["message"] = f"发现更新: {display_version}"
            else:
                update_status.update({
                    "status": "idle",
                    "message": "当前已是最新版本",
                    "update_info": None
                })
            return {"success": True, "update_found": found}

        elif operation_type == "apply":
            success = update_application.start_continuous_update(backup=backup)
            if success:
                update_status.update({
                    "status": "completed",
                    "progress": 100,
                    "message": "更新完成，请重启应用"
                })
                # 更新本地版本显示
                update_info = update_application.update_manager.get_update_info()
                if update_info:
                    new_version = update_info.get('target_version') or update_info.get('version') or update_application.version
                    update_application.version = new_version
            else:
                error_callback("更新失败")
            return {"success": success}

        elif operation_type == "rollback":
            success = update_application.rollback()
            if success:
                update_status.update({
                    "status": "completed",
                    "progress": 100,
                    "message": "回滚完成，请重启应用"
                })
            else:
                error_callback("回滚失败")
            return {"success": success}

        else:
            return {"success": False, "error": f"未知操作类型: {operation_type}"}

    except Exception as e:
        error_callback(str(e))
        return {"success": False, "error": str(e)}

@router.post("/check")
async def check_update():
    """检查更新"""
    if not update_lock.acquire(blocking=False):
        raise HTTPException(status_code=409, detail="已有更新操作在进行中")

    try:
        update_status.update({
            "status": "checking",
            "progress": 0,
            "message": "正在检查更新...",
            "error": None
        })

        def check():
            try:
                execute_update_operation("check")
            finally:
                update_lock.release()

        thread = threading.Thread(target=check)
        thread.daemon = True
        thread.start()

        return {"success": True, "message": "开始检查更新"}
    except Exception as e:
        update_lock.release()
        raise HTTPException(status_code=500, detail=f"启动检查更新失败: {str(e)}")

@router.post("/apply")
async def apply_update(request_data: ApplyUpdateRequest):
    """应用更新"""
    if not update_lock.acquire(blocking=False):
        raise HTTPException(status_code=409, detail="已有更新操作在进行中")

    backup = request_data.backup

    try:
        update_status.update({
            "status": "downloading",
            "progress": 0,
            "message": "开始下载更新...",
            "error": None
        })

        def apply():
            try:
                execute_update_operation("apply", backup=backup)
            finally:
                update_lock.release()

        thread = threading.Thread(target=apply)
        thread.daemon = True
        thread.start()

        return {"success": True, "message": "开始更新"}
    except Exception as e:
        update_lock.release()
        raise HTTPException(status_code=500, detail=f"启动更新失败: {str(e)}")

@router.post("/rollback")
async def rollback_update():
    """回滚更新"""
    if not update_lock.acquire(blocking=False):
        raise HTTPException(status_code=409, detail="已有更新操作在进行中")

    try:
        update_status.update({
            "status": "rolling_back",
            "progress": 0,
            "message": "正在回滚...",
            "error": None
        })

        def rollback():
            try:
                execute_update_operation("rollback")
            finally:
                update_lock.release()

        thread = threading.Thread(target=rollback)
        thread.daemon = True
        thread.start()

        return {"success": True, "message": "开始回滚"}
    except Exception as e:
        update_lock.release()
        raise HTTPException(status_code=500, detail=f"启动回滚失败: {str(e)}")

@router.get("/status")
async def get_update_status():
    """获取更新状态"""
    return update_status

@router.get("/info")
async def get_app_info():
    """获取应用信息"""
    update_available = update_application.update_manager.is_update_available()
    update_chain_info = None

    if update_available:
        try:
            chain_info = update_application.get_update_chain_info()
            if chain_info and chain_info.get('update_count', 0) > 1:
                update_chain_info = chain_info
        except Exception as e:
            print(f"获取更新链信息失败: {e}")

    return {
        "current_version": update_application.version,
        "update_available": update_available,
        "update_chain_info": update_chain_info
    }

@router.get("/config")
async def get_config():
    """获取更新配置"""
    return update_config

@router.post("/config")
async def update_config_route(config: UpdateConfig):
    """更新配置"""
    try:
        update_config["auto_backup"] = config.auto_backup
        update_config["auto_apply"] = config.auto_apply
        return {"success": True, "message": "配置已更新", "config": update_config}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@router.get("/health")
async def health_check():
    """健康检查"""
    return {"status": "ok", "message": "更新服务正常运行"}
