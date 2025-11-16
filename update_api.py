# update_api.py
from flask import Flask, request, jsonify
from flask_cors import CORS
from update_main import create_application
import threading
import time
import os

app = Flask(__name__)
# 添加CORS支持，允许前端跨域访问
CORS(app)

# 创建应用实例
application = create_application(
    version_file="version", 
    update_url="http://localhost:5100/updates"
)

# 全局状态存储
update_status = {
    "status": "idle",
    "progress": 0,
    "message": "",
    "update_info": None,
    "error": None
}

# 配置设置 - 重命名变量避免冲突
app_config = {
    "auto_backup": True,  # 默认自动备份
    "auto_apply": False   # 默认不自动应用
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

# 注册回调
application.update_manager.register_callback("status_changed", update_status_callback)
application.update_manager.register_callback("progress_changed", update_progress_callback)
application.update_manager.register_callback("update_available", update_available_callback)
application.update_manager.register_callback("update_completed", update_completed_callback)
application.update_manager.register_callback("error_occurred", error_callback)

def execute_update_operation(operation_type):
    """执行更新操作的统一入口"""
    try:
        if operation_type == "check":
            found = application.manual_check_update()
            if found:
                update_status["message"] = "发现新版本"
            else:
                update_status.update({
                    "status": "idle",
                    "message": "当前已是最新版本",
                    "update_info": None
                })
            return {"success": True, "update_found": found}
            
        elif operation_type == "apply":
            # 使用配置中的设置，而不是前端参数
            backup = app_config.get("auto_backup", True)
            success = application.start_update(backup=backup)
            if success:
                update_status.update({
                    "status": "completed",
                    "progress": 100,
                    "message": "更新完成，请重启应用"
                })
            else:
                error_callback("更新失败")
            return {"success": success}
            
        elif operation_type == "rollback":
            success = application.rollback()
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

@app.route('/api/update/check', methods=['POST'])
def check_update():
    """检查更新"""
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

@app.route('/api/update/apply', methods=['POST'])
def apply_update():
    """应用更新"""
    update_status.update({
        "status": "downloading",
        "progress": 0,
        "message": "开始下载更新...",
        "error": None
    })
    
    def apply():
        execute_update_operation("apply")
    
    thread = threading.Thread(target=apply)
    thread.daemon = True
    thread.start()
    
    return jsonify({"success": True, "message": "开始更新"})

@app.route('/api/update/rollback', methods=['POST'])
def rollback_update():
    """回滚更新"""
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

@app.route('/api/update/status', methods=['GET'])
def get_update_status():
    """获取更新状态"""
    return jsonify(update_status)

@app.route('/api/update/info', methods=['GET'])
def get_app_info():
    """获取应用信息"""
    return jsonify({
        "current_version": application.version,
        "update_available": application.update_manager.is_update_available()
    })

@app.route('/api/update/config', methods=['GET'])
def get_config():
    """获取更新配置"""
    return jsonify(app_config)

@app.route('/api/update/config', methods=['POST'])
def update_config():
    """更新配置"""
    try:
        data = request.get_json() or {}
        for key, value in data.items():
            if key in app_config:
                app_config[key] = value
        
        # 可选：保存配置到文件
        # with open('update_config.json', 'w') as f:
        #     json.dump(app_config, f)
            
        return jsonify({"success": True, "message": "配置已更新", "config": app_config})
    except Exception as e:
        return jsonify({"success": False, "error": str(e)})

@app.route('/api/update/health', methods=['GET'])
def health_check():
    """健康检查端点"""
    return jsonify({"status": "ok", "message": "服务正常运行"})

if __name__ == '__main__':
    print("启动更新API服务...")
    print("服务地址: http://localhost:5001")
    print("API文档:")
    print("  GET  /api/update/info    - 获取应用信息")
    print("  GET  /api/update/status  - 获取更新状态")
    print("  GET  /api/update/config  - 获取配置")
    print("  POST /api/update/check   - 检查更新")
    print("  POST /api/update/apply   - 应用更新")
    print("  POST /api/update/rollback - 回滚更新")
    print("  POST /api/update/config  - 更新配置")
    print("  GET  /api/update/health  - 健康检查")
    app.run(host='0.0.0.0', port=5001, debug=True)