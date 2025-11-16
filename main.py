import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import threading
from flask import Flask, request, jsonify
from flask_cors import CORS
from update_main import create_application
import logging

update_app = Flask(__name__)
CORS(update_app)
logging.getLogger('werkzeug').setLevel(logging.ERROR)
logging.getLogger('werkzeug').disabled = True
update_app.logger.disabled = True

update_application = create_application(
    version_file="version", 
    update_url="http://localhost:5100/updates"
)
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

@update_app.route('/api/update/check', methods=['POST'])
def check_update():
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
    
    return jsonify({"success": True, "message": "开始检查更新"})

@update_app.route('/api/update/apply', methods=['POST'])
def apply_update():
    data = request.get_json() or {}
    backup = data.get('backup', update_config.get("auto_backup", True))
    
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
    
    return jsonify({"success": True, "message": "开始更新"})

@update_app.route('/api/update/rollback', methods=['POST'])
def rollback_update():
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
    
    return jsonify({"success": True, "message": "开始回滚"})

@update_app.route('/api/update/status', methods=['GET'])
def get_update_status():
    return jsonify(update_status)

@update_app.route('/api/update/info', methods=['GET'])
def get_app_info():
    update_available = update_application.update_manager.is_update_available()
    update_chain_info = None
    
    if update_available:
        try:
            chain_info = update_application.get_update_chain_info()
            if chain_info and chain_info.get('update_count', 0) > 1:
                update_chain_info = chain_info
        except Exception as e:
            print(f"获取更新链信息失败: {e}")
    
    return jsonify({
        "current_version": update_application.version,
        "update_available": update_available,
        "update_chain_info": update_chain_info
    })

@update_app.route('/api/update/config', methods=['GET'])
def get_config():
    return jsonify(update_config)

@update_app.route('/api/update/config', methods=['POST'])
def update_config_route():
    try:
        data = request.get_json() or {}
        for key, value in data.items():
            if key in update_config:
                update_config[key] = value
        return jsonify({"success": True, "message": "配置已更新", "config": update_config})
    except Exception as e:
        return jsonify({"success": False, "error": str(e)})

@update_app.route('/api/update/health', methods=['GET'])
def health_check():
    return jsonify({"status": "ok", "message": "服务正常运行"})

def start_update_api():
    import werkzeug.serving
    original_log_request = werkzeug.serving.WSGIRequestHandler.log_request
    
    def silent_log_request(self, *args, **kwargs):
        pass 
    werkzeug.serving.WSGIRequestHandler.log_request = silent_log_request
    
    try:
        print("更新API服务已启动 (端口: 5001)")
        update_app.run(
            host='0.0.0.0', 
            port=5001, 
            debug=False, 
            use_reloader=False
        )
    finally:
        werkzeug.serving.WSGIRequestHandler.log_request = original_log_request

def run_application():
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

    api_thread = threading.Thread(target=start_update_api, daemon=True)
    api_thread.start()
    
    print("更新API服务已在后台启动 (端口: 5001)")
    print("主应用开始运行...")
    from ling_chat import main
    main.main()

if __name__ == "__main__":
    run_application()