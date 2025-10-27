import os
import webview
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path

# 创建一个类以向JavaScript暴露函数
class Api:
    def __init__(self):
        self._window = None

    def set_window(self, window):
        self._window = window

    def toggle_fullscreen(self):
        if self._window:
            self._window.toggle_fullscreen()

def start_webview():
    try:
        api = Api()
        window = webview.create_window(
            "Ling Chat",
            url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/",
            width=1024,
            height=600,
            resizable=True,
            fullscreen=False,
            js_api=api  # 向JavaScript暴露Api类
        )
        api.set_window(window)  # 让Api实例能够访问window对象

        webview.start(
            http_server=True,
            icon=str(static_path / "game_data/resources/lingchat.ico"),
            storage_path=str(user_data_path / "webview_storage_path")
        )
    except KeyboardInterrupt:
        logger.info("WebView被中断")
