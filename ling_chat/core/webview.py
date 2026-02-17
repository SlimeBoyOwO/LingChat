import os
import multiprocessing
import webview

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path


# 创建一个类以向JavaScript暴露函数
class Api:
    def __init__(self):
        self._window = None

    def set_window(self, window) -> None:
        self._window = window

    def toggle_fullscreen(self):
        if self._window:
            self._window.toggle_fullscreen()

def func_webview():
    try:
                    # 设置环境变量强制使用Qt后端并完全禁用OpenGL
        os.environ['PYWEBVIEW_GUI'] = 'qt'
        os.environ['QT_DEBUG_PLUGINS'] = '0'  # 关闭插件调试
        os.environ['QT_WEBENGINE_CHROMIUM_FLAGS'] = '--disable-gpu '#--disable-webgl --disable-accelerated-2d-canvas --disable-accelerated-video-decode'
        os.environ['QT_QUICK_BACKEND'] = 'software'
        os.environ['QT_XCB_GL_INTEGRATION'] = 'none'  # 禁用XCB OpenGL集成
        api:Api = Api()
        window = webview.create_window(
            "Ling Chat",
            url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/",
            width=1024,
            height=600,
            resizable=True,
            fullscreen=False,
            js_api=api,  # 向JavaScript暴露Api类
        )
        api.set_window(window)  # 让Api实例能够访问window对象

        icon_path = static_path / "game_data/resources/lingchat.ico"

        # print(f"图标路径: {icon_path}")
        # print(f"图标文件是否存在: {icon_path.exists()}")

        webview.start(
            http_server=True,
            icon=str(icon_path),  # 在这里设置图标
            storage_path=str(user_data_path / "webview_storage_path")
        )

    except KeyboardInterrupt:
        logger.info("WebView被中断")


def start_webview():
    webview_process = multiprocessing.Process(target=func_webview)
    webview_process.start()
    webview_process.join()
    return webview_process