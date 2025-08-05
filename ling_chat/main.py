import os
import threading
import subprocess
import zipfile
import signal
import sys

import uvicorn
import webview
import py7zr
from pathlib import Path
from fastapi import FastAPI, Request

from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import static_path, user_data_path, third_party_path

if os.path.exists(".env"):
    Function.load_env()
else:
    Function.load_env(".env.example")
    Function.load_env(user_data_path / ".env" , init=True) # 加载用户数据目录下的环境变量

from ling_chat.api.routes_manager import RoutesManager
from ling_chat.core.logger import logger, TermColors
from ling_chat.database import init_db
from ling_chat.database.character_model import CharacterModel

app = FastAPI()

def init_system():
    try:
        logger.info("正在初始化数据库...")
        init_db()

        logger.info("正在同步游戏角色数据...")
        CharacterModel.sync_characters_from_game_data(user_data_path / "game_data")

        logger.stop_loading_animation(success=True, final_message="应用加载成功")

    except (ImportError, Exception) as e:
        logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        raise e

init_system()  # 初始化系统

@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next):
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update(
            {"Cache-Control": "no-cache, no-store, must-revalidate", "Pragma": "no-cache", "Expires": "0"})
    return response


routes_manager = RoutesManager(app)    # 挂载路由
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
        logger.info("正在启动HTTP服务器...")
        config = uvicorn.Config(app, host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
                                port=int(os.getenv('BACKEND_PORT', '8765')), log_level=os.getenv("LOG_LEVE","info").lower())
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
    print(f"正在解压 {archive_path} 到 {extract_to}...")

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
    archive_path = vits_path.parent
    if not vits_path.exists():
        vits_archive = archive_path / "vits-simple-api-windows-cpu-v0.6.16.7z"
        assert vits_archive.exists(), f"VITS语音合成器压缩包未找到，请手动下载到{vits_archive}。"
        extract_archive(vits_archive, vits_path)

    assert vits_path.exists(), "VITS语音合成器目录未找到，请检查解压是否成功。"

    vits_model_path = vits_path / "data/models/YuzuSoft_Vits"
    if not vits_model_path.exists():
        vits_model_archive = archive_path / "YuzuSoft_Vits.zip"
        assert vits_model_archive.exists(), f"VITS模型压缩包未找到，请手动下载到{vits_model_archive}。"
        extract_archive(vits_model_archive, vits_model_path)

    assert vits_model_path.exists(), "VITS模型目录未找到，请检查解压是否成功。"

def run_vits_process(vits_ready_event: threading.Event, status_ok: list[bool]):
    try:
        vits_path = third_party_path / "vits-simple-api/vits-simple-api-windows-cpu-v0.6.16"
        prepare_vits_directory(vits_path)

        vits_process = subprocess.Popen(
            [vits_path / "py310/python.exe", "app.py"],
            cwd=vits_path,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            bufsize=1,  # 行缓冲
            text=True  # 文本模式
        )
        if vits_process.stdout:
            for line in vits_process.stdout:
                logger.info(f'[vits]: {line.strip()}')
                if "* Running on http:" in line:
                    status_ok[0] = True
                    vits_ready_event.set()
    except Exception as e:
        logger.error(f"启动VITS语音合成器失败: {e}")
        status_ok[0] = False
        vits_ready_event.set()


def start_webview():
    try:
        webview.create_window(
            "Ling Chat", url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/",
            width=1024, height=600,
            resizable=True, fullscreen=False
        )
        webview.start(
            http_server=True,
            icon=str(static_path / "game_data/resources/lingchat.ico"),
            storage_path=str(user_data_path / "webview_storage_path"),
        )
    except KeyboardInterrupt:
        logger.info("WebView被中断")

should_exit = False
def signal_handler(signum, frame):
    """处理中断"""
    global should_exit
    logger.info("接收到中断信号，正在关闭程序...")
    should_exit = True
    if 'app_server' in globals():
        app_server.should_exit = True
    sys.exit(0)

def main():
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    print_logo()
    vits_ready_event = threading.Event()
    status_ok = [False]  # 用于检查VITS是否成功启动
    vits_thread = threading.Thread(target=run_vits_process, args=(vits_ready_event, status_ok), daemon=True)
    vits_thread.start()  # 启动 vits
    if not vits_ready_event.wait(timeout=120) and not status_ok[0]:
        logger.error("VITS语音合成器未能在30秒内启动，请检查配置或手动启动。")
    else:
        logger.info("VITS语音合成器已成功启动。")

    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()  # 启动 Uvicorn 服务器线程

    # 检查环境变量决定是否启动前端界面
    if os.getenv('OPEN_FRONTEND_APP', 'false').lower() == "true":
        try:
            start_webview()
        except KeyboardInterrupt:
            logger.info("用户关闭程序")
    else:
        logger.info("已根据环境变量禁用前端界面")
        # 让主线程等待并保持后端服务运行
        try:
            while not should_exit:
                app_thread.join(timeout=1)
        except KeyboardInterrupt:
            logger.info("正在关闭服务...")
            signal_handler(signal.SIGINT, None)

    # 确保服务器关闭
    if 'app_server' in globals():
        app_server.should_exit = True
    app_thread.join(timeout=5)


if __name__ == "__main__":
    main()