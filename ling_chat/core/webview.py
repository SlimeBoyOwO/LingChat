import os

import webview

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path


def start_webview():
    try:
        webview.create_window("Ling Chat", url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/", width=1024,
            height=600, resizable=True, fullscreen=False)
        webview.start(http_server=True, icon=str(static_path / "game_data/resources/lingchat.ico"),
            storage_path=str(user_data_path / "webview_storage_path"), )
    except KeyboardInterrupt:
        logger.info("WebView被中断")
