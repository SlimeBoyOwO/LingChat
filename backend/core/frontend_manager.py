# frontend_manager.py
import os
import subprocess
import webbrowser
import time
import atexit
import signal
import sys
from typing import Optional
from .logger import log_debug, log_info, log_warning, log_error

class FrontendManager:
    def __init__(self, logger=None):
        self.process: Optional[subprocess.Popen] = None
        self._cleanup_called = False
        self.logger = logger  # 保留参数以兼容旧代码，但不再使用
        self._register_cleanup()

    def start_frontend(self, frontend_dir: str, port: str = "3000") -> bool:
        """启动前端开发服务器"""
        server_js_path = os.path.join(frontend_dir, "app.js")
        
        if not os.path.isfile(server_js_path):
            log_error(f"前端脚本未找到: {server_js_path}")
            return False

        try:
            self.process = subprocess.Popen(
                ["node", os.path.basename(server_js_path)],
                cwd=frontend_dir,
                creationflags=self._get_creation_flags(),
                start_new_session=sys.platform != "win32"
            )
            log_info(f"前端服务器已启动 (PID: {self.process.pid})")
            
            time.sleep(3)  # 等待服务器启动
            if self.process.poll() is not None:
                log_error(f"前端服务器退出，错误码: {self.process.poll()}")
                return False
            
            self._open_browser(f"http://localhost:{port}")
            return True
            
        except FileNotFoundError:
            log_error("未找到'node'命令. 请安装Node.js")
            return False
        except Exception as e:
            log_error(f"启动前端失败: {e}")
            return False

    def stop_frontend(self):
        """停止前端服务器"""
        if self._cleanup_called or not self.process:
            return

        self._cleanup_called = True
        log_info("正在停止前端服务器...")
        
        try:
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)
        except Exception as e:
            log_error(f"停止前端出错: {e}")
        finally:
            self.process = None

    def _open_browser(self, url: str):
        """尝试在浏览器中打开URL"""
        try:
            webbrowser.open(url)
            log_info(f"已在浏览器中打开: {url}")
        except Exception as e:
            log_error(f"打开浏览器失败: {e}")

    def _get_creation_flags(self):
        """获取平台特定的进程创建标志"""
        return subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == "win32" else 0

    def _register_cleanup(self):
        """注册清理函数"""
        atexit.register(self.stop_frontend)
        
        # 注册信号处理器
        signals = (signal.SIGINT, signal.SIGTERM)
        if sys.platform != "win32":
            signals += (signal.SIGHUP,)
        else:
            signals += (signal.SIGBREAK,)

        for sig in signals:
            try:
                signal.signal(sig, self._handle_signal)
            except (OSError, ValueError, AttributeError) as e:
                log_warning(f"无法注册信号 {sig}: {e}")

    def _handle_signal(self, sig, frame):
        """处理操作系统信号"""
        log_info(f"收到信号 {signal.Signals(sig).name}，正在关闭...")
        self.stop_frontend()
        sys.exit(0)