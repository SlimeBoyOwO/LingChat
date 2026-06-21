import multiprocessing
import os
import platform

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path

# 尝试导入 webview，可选依赖缺失时降级 Server 模式
try:
    import webview

    WEBVIEW_AVAILABLE = True
except ImportError:
    logger.warning("pywebview 未安装，降级为 Server 模式（无本地窗口）")
    webview = None
    WEBVIEW_AVAILABLE = False


# 创建一个类以向JavaScript暴露函数
class Api:
    def __init__(self):
        self._window = None

    def set_window(self, window) -> None:
        self._window = window

    def toggle_fullscreen(self):
        if self._window:
            self._window.toggle_fullscreen()

    def exit_app(self):
        """优雅地退出应用，关闭窗口"""
        logger.info("收到退出请求，正在关闭应用...")
        if self._window:
            self._window.destroy()


def _get_gui_backends():
    """返回按优先级排列的 pywebview GUI 后端列表

    Windows 上优先使用 edgechromium（WebView2，Win10 19041+ / Win11 自带），
    然后依次降级到 edgehtml、cef，最后回退到 winforms（旧行为）。
    某些后端（如 winforms）依赖 pythonnet 加载 Python.Runtime.dll，
    与嵌入式 Python 3.13 不兼容，所以放在最后兜底。
    """
    system = platform.system()
    if system == "Windows":
        return ["edgechromium", "edgehtml", "cef", "winforms"]
    elif system == "Darwin":
        return ["cocoa"]
    else:
        return ["gtk", "qt"]


def _migrate_webview_storage():
    """后端从 winforms 切换到 edgechromium 后，旧缓存格式不兼容。

    检测旧缓存目录中是否存在 winforms 特征文件（如 .NET 相关），
    如果有则将旧目录重命名备份，让新后端从干净状态启动。
    避免新后端读取不兼容的旧缓存导致异常。
    """
    storage_dir = user_data_path / "webview_storage_path"
    if not storage_dir.exists():
        return

    # winforms 后端会在 storage_path 下生成 .NET WebBrowser 缓存结构
    # edgechromium 后端使用 Chromium 的 WebView2 用户数据目录结构
    # 通过检测是否缺少 EBWebView 目录（edgechromium 特征）来判断是否为旧缓存
    ebwebview_dir = storage_dir / "EBWebView"
    if ebwebview_dir.exists():
        # 已经是 edgechromium 格式，无需迁移
        return

    # 目录存在但不是 edgechromium 格式 → 旧后端遗留，备份后清空
    legacy_dir = user_data_path / "webview_storage_path_legacy"
    if legacy_dir.exists():
        # 已经备份过一次，直接删除当前目录内容让新后端重建
        import shutil

        shutil.rmtree(storage_dir, ignore_errors=True)
        logger.info("已清理旧 webview 缓存目录")
    else:
        storage_dir.rename(legacy_dir)
        logger.info(
            f"检测到旧版 webview 缓存，已备份到 {legacy_dir.name}，新后端将重新初始化"
        )


def func_webview():
    try:
        _migrate_webview_storage()
        api: Api = Api()

        frontend_bind_addr = os.getenv("FRONTEND_BIND_ADDR") or os.getenv(
            "BACKEND_BIND_ADDR", "127.0.0.1"
        )
        frontend_port = os.getenv("FRONTEND_PORT") or os.getenv("BACKEND_PORT", "8765")

        window = webview.create_window(
            "Ling Chat",
            url=f"http://{frontend_bind_addr}:{frontend_port}/",
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

        backends = _get_gui_backends()
        started = False

        for gui in backends:
            try:
                logger.info(f"尝试使用 pywebview 后端: {gui}")
                webview.start(
                    gui=gui,
                    http_server=True,
                    icon=str(icon_path),  # 在这里设置图标
                    storage_path=str(user_data_path / "webview_storage_path"),
                )
                started = True
                break
            except Exception as e:
                logger.warning(
                    f"pywebview 后端 '{gui}' 启动失败: {e}，尝试下一个后端..."
                )
                continue

        if not started:
            logger.error(
                "所有 pywebview 后端均启动失败。"
                f"请打开浏览器访问 http://{frontend_bind_addr}:{frontend_port}/"
            )
            # 用非零退出码通知主进程，便于主进程提示用户
            raise SystemExit(1)

    except KeyboardInterrupt:
        logger.info("WebView被中断")


def start_webview():
    if not WEBVIEW_AVAILABLE:
        logger.info("pywebview 不可用，跳过本地窗口（Server 模式）")
        return None

    webview_process = multiprocessing.Process(target=func_webview)
    webview_process.start()
    webview_process.join()
    return webview_process
