import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import threading
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import uvicorn
from ling_chat.update.update_main import create_application
import logging

# 创建 FastAPI 应用
update_app = FastAPI(title="Update API", docs_url=None, redoc_url=None)

# 配置 CORS
update_app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 禁用日志
logging.getLogger('uvicorn.access').disabled = True
logging.getLogger('uvicorn.error').disabled = True
logging.getLogger('uvicorn').disabled = True

# 初始化更新应用
update_application = create_application(
    version_file="version", 
    update_url="http://localhost:5100/updates"
)

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

def execute_update_operation(operation_type, backup=True):
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

@update_app.post("/api/update/check")
async def check_update():
    update_status.update({
        "status": "checking",
        "progress": 0,
        "message": "正在检查更新...",
        "error": None
    })

    def check():
        execute_update_operation("check")

    thread = threading.Thread(target=check)
    thread.daemon = True
    thread.start()

    return {"success": True, "message": "开始检查更新"}

@update_app.post("/api/update/apply")
async def apply_update(request_data: ApplyUpdateRequest):
    backup = request_data.backup

    update_status.update({
        "status": "downloading",
        "progress": 0,
        "message": "开始下载更新...",
        "error": None
    })

    def apply():
        execute_update_operation("apply", backup=backup)

    thread = threading.Thread(target=apply)
    thread.daemon = True
    thread.start()

    return {"success": True, "message": "开始更新"}

@update_app.post("/api/update/rollback")
async def rollback_update():
    update_status.update({
        "status": "rolling_back",
        "progress": 0,
        "message": "正在回滚...",
        "error": None
    })

    def rollback():
        execute_update_operation("rollback")

    thread = threading.Thread(target=rollback)
    thread.daemon = True
    thread.start()

    return {"success": True, "message": "开始回滚"}

@update_app.get("/api/update/status")
async def get_update_status():
    return update_status

@update_app.get("/api/update/info")
async def get_app_info():
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

@update_app.get("/api/update/config")
async def get_config():
    return update_config

@update_app.post("/api/update/config")
async def update_config_route(config: UpdateConfig):
    try:
        update_config["auto_backup"] = config.auto_backup
        update_config["auto_apply"] = config.auto_apply
        return {"success": True, "message": "配置已更新", "config": update_config}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@update_app.get("/api/update/health")
async def health_check():
    return {"status": "ok", "message": "服务正常运行"}

def start_update_api():
    """启动更新 API 服务"""
    config = uvicorn.Config(
        update_app,
        host="0.0.0.0",
        port=5001,
        log_config=None,
        access_log=False
    )
    server = uvicorn.Server(config)
    
    print("更新API服务已启动 (端口: 5001)")
    server.run()

def run_application():
    """运行主应用程序"""
    # 禁用 uvicorn 日志
    uvicorn_loggers = [
        "uvicorn",
        "uvicorn.access",
        "uvicorn.error",
        "uvicorn.asgi"
    ]

    for logger_name in uvicorn_loggers:
        logger = logging.getLogger(logger_name)
        logger.disabled = True
        logger.propagate = False

    # 启动 API 服务线程
    api_thread = threading.Thread(target=start_update_api, daemon=True)
    api_thread.start()

    print("更新API服务已在后台启动 (端口: 5001)")
    print("主应用开始运行...")
    
    # 启动主应用
    from ling_chat import main
    main.main()

if __name__ == "__main__":
    run_application()