# frontend_manager.py
import os
import subprocess
import webbrowser
import time
import atexit
import signal
import sys
from typing import Optional

class FrontendManager:
    def __init__(self):
        self.process: Optional[subprocess.Popen] = None
        self._cleanup_called = False
        self._register_cleanup()

    def start_frontend(self, frontend_dir: str, port: str = "3000") -> bool:
        """启动前端开发服务器"""
        server_js_path = os.path.join(frontend_dir, "app.js")
        
        if not os.path.isfile(server_js_path):
            print(f"Error: Frontend script not found at {server_js_path}")
            return False

        try:
            self.process = subprocess.Popen(
                ["node", os.path.basename(server_js_path)],
                cwd=frontend_dir,
                creationflags=self._get_creation_flags(),
                start_new_session=sys.platform != "win32"
            )
            print(f"Frontend server started (PID: {self.process.pid})")
            
            time.sleep(3)  # 等待服务器启动
            if self.process.poll() is not None:
                print(f"Frontend server exited with code {self.process.poll()}")
                return False
            
            self._open_browser(f"http://localhost:{port}")
            return True
            
        except FileNotFoundError:
            print("Error: 'node' command not found. Please install Node.js.")
            return False
        except Exception as e:
            print(f"Failed to start frontend: {e}")
            return False

    def stop_frontend(self):
        """停止前端服务器"""
        if self._cleanup_called or not self.process:
            return

        self._cleanup_called = True
        print("\nStopping frontend server...")
        
        try:
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)
        except Exception as e:
            print(f"Error stopping frontend: {e}")
        finally:
            self.process = None

    def _open_browser(self, url: str):
        """尝试在浏览器中打开URL"""
        try:
            webbrowser.open(url)
            print(f"Opened browser at {url}")
        except Exception as e:
            print(f"Failed to open browser: {e}")

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
                print(f"Warning: Could not register signal {sig}: {e}")

    def _handle_signal(self, sig, frame):
        """处理操作系统信号"""
        print(f"\nReceived signal {signal.Signals(sig).name}, shutting down...")
        self.stop_frontend()
        sys.exit(0)