import os
import threading
import subprocess
import zipfile

import uvicorn
import webview
import py7zr
from pathlib import Path
from dotenv import load_dotenv
from fastapi import FastAPI, Request

from ling_chat.api.chat_main import websocket_endpoint
from ling_chat.api.chat_history import router as chat_history_router
from ling_chat.api.chat_info import router as chat_info_router
from ling_chat.api.chat_character import router as chat_character_router
from ling_chat.api.chat_music import router as chat_music_router
from ling_chat.api.chat_background import router as chat_background_router
from ling_chat.api.frontend_routes import router as frontend_router, get_static_files, get_audio_files
from ling_chat.core.logger import logger, TermColors
from ling_chat.database import init_db
from ling_chat.database.character_model import CharacterModel
from ling_chat.utils.runtime_path import static_path, user_data_path

load_dotenv(".env.example")
load_dotenv()
load_dotenv(user_data_path / ".env")  # 加载用户数据目录下的环境变量

app = FastAPI()

def init_system():
    try:
        logger.info("正在初始化数据库...")
        init_db()

        logger.info("正在同步游戏角色数据...")
        charaModel = CharacterModel()
        charaModel.sync_characters_from_game_data(str(static_path))

        logger.stop_loading_animation(success=True, final_message="应用加载成功")

    except (ImportError, Exception) as e:
        logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        raise e


@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next):
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update(
            {"Cache-Control": "no-cache, no-store, must-revalidate", "Pragma": "no-cache", "Expires": "0"})
    return response


# 注册路由
app.include_router(chat_history_router)
app.include_router(chat_info_router)
app.include_router(frontend_router)
app.include_router(chat_music_router)
app.include_router(chat_character_router)
app.include_router(chat_background_router)

app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
frontend_dir = static_path.resolve()
app.mount("/audio", get_audio_files(), name="audio")  # 托管audio文件
app.mount("/", get_static_files(), name="static")  # 托管静态文件
logger.info_color("所有组件初始化完毕，服务器准备就绪。", color=TermColors.CYAN)


def print_logo():
    logo = [  #
        "█╗       ██╗ ███╗   ██╗  ██████╗      █████╗ ██╗  ██╗  █████╗  ████████╗",
        "██║      ██║ ████╗  ██║ ██╔════╝     ██╔═══╝ ██║  ██║ ██╔══██╗ ╚══██╔══╝",
        "██║      ██║ ██╔██╗ ██║ ██║  ███╗    ██║     ███████║ ███████║    ██║   ",
        "██║      ██║ ██║╚██╗██║ ██║   ██║    ██║     ██╔══██║ ██╔══██║    ██║   ",
        "███████╗ ██║ ██║ ╚████║ ╚██████╔╝     █████╗ ██║  ██║ ██║  ██║    ██║   ",
        "╚══════╝ ╚═╝ ╚═╝  ╚═══╝  ╚═════╝      ╚════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝    ╚═╝   ",  #
    ]
    for line in logo:
        print(line)


app_server: uvicorn.Server


def run_app():
    try:
        print("正在启动HTTP服务器...")
        config = uvicorn.Config(app, host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
                                port=int(os.getenv('BACKEND_PORT', '8765')), log_level="info")
        global app_server
        app_server = uvicorn.Server(config)
        app_server.run()
    except Exception as e:
        logger.error(f"服务器启动错误: {e}")


def extract_archive(archive_path: Path, extract_to: Path):
    """
    解压压缩文件到指定目录，支持7z和zip格式

    :param archive_path: 压缩文件路径(7z或zip)
    :param extract_to: 解压目标目录
    :raises ValueError: 当文件格式不支持时
    """
    # 确保目标目录存在
    extract_to.mkdir(parents=True, exist_ok=True)

    # 根据后缀选择解压方式
    suffix = archive_path.suffix.lower()

    if suffix == '.7z':
        with py7zr.SevenZipFile(archive_path, mode='r') as z:
            z.extractall(path=extract_to)
    elif suffix == '.zip':
        with zipfile.ZipFile(archive_path, 'r') as z:
            z.extractall(path=extract_to)
    else:
        raise ValueError(f"不支持的压缩格式: {suffix}. 仅支持 .7z 和 .zip")

    print(f"成功解压 {archive_path} 到 {extract_to}")

def prepare_vits_directory(vits_path: Path):
    if not vits_path.exists():
        vits_archive = static_path / "vits-simple-api-windows-cpu-v0.6.16.7z"
        assert vits_archive.exists(), f"VITS语音合成器压缩包未找到，请手动下载到{vits_archive}。"
        extract_archive(vits_archive, vits_path)

    assert vits_path.exists(), "VITS语音合成器目录未找到，请检查解压是否成功。"

    vits_model_path = vits_path / "data/models/YuzuSoft_Vits"
    if not vits_model_path.exists():
        vits_model_archive = static_path / "YuzuSoft_Vits.zip"
        assert vits_model_archive.exists(), f"VITS模型压缩包未找到，请手动下载到{vits_model_archive}。"
        extract_archive(vits_model_archive, vits_model_path)

    assert vits_model_path.exists(), "VITS模型目录未找到，请检查解压是否成功。"

def start_vits_process():
    try:
        vits_path = static_path / "vits-simple-api-windows-cpu-v0.6.16"
        prepare_vits_directory(vits_path)

        vits_process = subprocess.Popen(
            ["py310/python.exe", "app.py"],
            cwd=vits_path
            # stdout=subprocess.PIPE,
            # stderr=subprocess.PIPE
        )
        return vits_process
    except Exception as e:
        logger.error(f"启动VITS语音合成器失败: {e}")
        return None


def start_webview():
    webview.create_window(
        "Ling Chat", url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/",
        width=1024, height=768,
        resizable=True, fullscreen=False
    )
    webview.start(http_server=True, icon=str(static_path / "resources/lingchat.ico"))


def main():
    print_logo()
    start_vits_process()

    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()  # 启动 Uvicorn 服务器线程

    start_webview()

    app_server.should_exit = True  # 停止 Uvicorn 服务器
    app_thread.join()  # 等待线程结束


if __name__ == "__main__":
    main()
