import logging
import os
import sys
import threading
import time
from datetime import datetime

# 日志配置
ENABLE_FILE_LOGGING = True  # 是否启用文件日志记录
LOG_FILE_DIRECTORY = "run_logs"  # 日志文件存储的相对目录
LOG_FILE_LEVEL = (
    logging.DEBUG
)  # 可以设置为 logging.INFO, logging.WARNING, logging.ERROR

ANIMATION_STYLES = {
    "braille": ["⢿", "⣻", "⣽", "⣾", "⣷", "⣯", "⣟", "⡿"],
    "spinner": ["-", "\\", "|", "/"],
    "dots": [".  ", ".. ", "...", " ..", "  .", "   "],
    "arrows": ["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
    "moon": ["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"],
    "clock": ["🕛", "🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚"],
    "directional_arrows_unicode": ["⬆️", "↗️", "➡️", "↘️", "⬇️", "↙️", "⬅️", "↖️"],
    "traffic_lights": ["🔴", "🟡", "🟢"],
    "growth_emoji": ["🌱", "🌿", "🌳"],
    "weather_icons": ["☀️", "☁️", "🌧️", "⚡️"],
    "heartbeat": ["♡", "♥"],
}

sys.stderr.flush()


def wcswidth(s):
    """回退 wcswidth, 将非 ASCII 字符视为宽度2。"""
    if not isinstance(s, str):
        return len(s) if s else 0
    length = 0
    for char_ in s:
        if ord(char_) < 128:
            length += 1
        else:
            length += 2
    return length


class TermColors:
    GREY = "\033[90m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BLUE = "\033[94m"
    RESET = "\033[0m"
    WHITE = "\033[97m"
    CYAN = "\033[96m"
    MAGENTA = "\033[95m"
    LIGHT_BLUE = "\033[94m"
    ORANGE = "\033[38;5;208m"


_logger = None
_animation_thread = None
_stop_animation_event = threading.Event()

_is_animating = False
_current_animation_line_width = 0
_animation_state_lock = threading.Lock()

DEFAULT_ANIMATION_STYLE_KEY = "braille"
DEFAULT_ANIMATION_COLOR = TermColors.WHITE


class AnimationAwareStreamHandler(logging.StreamHandler):
    def emit(self, record):
        global _is_animating, _current_animation_line_width, _animation_state_lock

        if hasattr(record, "is_animation_control") and record.is_animation_control:
            super().emit(record)
            return

        with _animation_state_lock:
            is_currently_animating = _is_animating
            animation_width_to_clear = _current_animation_line_width

        if is_currently_animating and animation_width_to_clear > 0:
            self.acquire()
            try:
                self.flush()
                self.stream.write("\r" + " " * animation_width_to_clear + "\r")
                self.stream.flush()
            finally:
                self.release()

        super().emit(record)


class ColoredFormatter(logging.Formatter):
    DATE_FORMAT = "%Y-%m-%d-%H:%M:%S"

    def __init__(self, show_timestamp=True):
        super().__init__(datefmt=self.DATE_FORMAT)
        self.show_timestamp = show_timestamp

    def format(self, record):
        if hasattr(record, "is_animation_control") and record.is_animation_control:
            return record.getMessage()

        timestamp_part = ""
        if self.show_timestamp:
            timestamp_str = self.formatTime(record, self.DATE_FORMAT)
            timestamp_part = f"{timestamp_str} "

        message_content = record.getMessage()
        level_name = record.levelname
        level_prefix_text = f"[{level_name}]: "

        if record.levelno == logging.DEBUG:
            return f"{TermColors.GREY}{timestamp_part}{level_prefix_text}{message_content}{TermColors.RESET}"

        level_color = ""
        if record.levelno == logging.INFO:
            level_color = TermColors.GREEN
        elif record.levelno == logging.WARNING:
            level_color = TermColors.YELLOW
        elif record.levelno == logging.ERROR:
            level_color = TermColors.RED

        colored_level_prefix = f"{level_color}{level_prefix_text}{TermColors.RESET}"
        return f"{timestamp_part}{colored_level_prefix}{message_content}"


def _animate(
    message="Loading", animation_chars=None, color_code=DEFAULT_ANIMATION_COLOR
):
    global \
        _is_animating, \
        _current_animation_line_width, \
        _animation_state_lock, \
        _stop_animation_event

    if animation_chars is None:
        animation_chars = ANIMATION_STYLES[DEFAULT_ANIMATION_STYLE_KEY]

    idx = 0
    last_char_for_clear = animation_chars[0]

    while not _stop_animation_event.is_set():
        char = animation_chars[idx % len(animation_chars)]
        last_char_for_clear = char

        visible_animation_text = f"{message} {char} "
        current_width = wcswidth(visible_animation_text)

        with _animation_state_lock:
            _current_animation_line_width = current_width

        sys.stdout.write(f"\r{color_code}{message} {char}{TermColors.RESET} ")
        sys.stdout.flush()

        idx += 1
        time.sleep(0.12)

    final_visible_text = f"{message} {last_char_for_clear} "
    width_to_clear = wcswidth(final_visible_text)

    sys.stdout.write("\r" + " " * width_to_clear + "\r")
    sys.stdout.flush()

    with _animation_state_lock:
        _is_animating = False
        _current_animation_line_width = 0


def start_loading_animation(
    message="Processing",
    animation_style_key=DEFAULT_ANIMATION_STYLE_KEY,
    animation_color=DEFAULT_ANIMATION_COLOR,
):
    global \
        _animation_thread, \
        _stop_animation_event, \
        _is_animating, \
        _animation_state_lock

    with _animation_state_lock:
        if _is_animating:
            return
        _is_animating = True

    _stop_animation_event.clear()

    selected_chars = ANIMATION_STYLES.get(
        animation_style_key, ANIMATION_STYLES[DEFAULT_ANIMATION_STYLE_KEY]
    )

    _animation_thread = threading.Thread(
        target=_animate, args=(message, selected_chars, animation_color), daemon=True
    )
    _animation_thread.start()


def stop_loading_animation(success=True, final_message=None):
    global \
        _animation_thread, \
        _stop_animation_event, \
        _is_animating, \
        _animation_state_lock

    acquire_lock = False
    with _animation_state_lock:
        if _is_animating or _animation_thread is not None:
            acquire_lock = True

    if not acquire_lock:
        return

    _stop_animation_event.set()

    current_thread_ref = _animation_thread
    if current_thread_ref and current_thread_ref.is_alive():
        current_thread_ref.join(timeout=2)

    with _animation_state_lock:
        _is_animating = False
        _current_animation_line_width = 0
        _animation_thread = None

    if final_message:
        if success:
            log_info(f"{TermColors.GREEN}✔{TermColors.RESET} {final_message}")
        else:
            log_error(f"{TermColors.RED}✖{TermColors.RESET} {final_message}")


def initialize_logger(
    app_name="AppLogger", config_debug_mode=True, show_timestamp=True
):
    global _logger
    _logger = logging.getLogger(app_name)
    _logger.propagate = False

    if config_debug_mode:
        _logger.setLevel(logging.DEBUG)
    else:
        _logger.setLevel(logging.INFO)

    if _logger.hasHandlers():
        for handler in _logger.handlers[:]:
            _logger.removeHandler(handler)
            handler.close()

    console_handler = AnimationAwareStreamHandler(sys.stdout)
    console_formatter = ColoredFormatter(show_timestamp=show_timestamp)
    console_handler.setFormatter(console_formatter)
    _logger.addHandler(console_handler)

    if ENABLE_FILE_LOGGING:
        try:
            if not os.path.exists(LOG_FILE_DIRECTORY):
                os.makedirs(LOG_FILE_DIRECTORY, exist_ok=True)

            log_filename = datetime.now().strftime("%Y-%m-%d_%H-%M-%S.log")
            log_filepath = os.path.join(LOG_FILE_DIRECTORY, log_filename)

            file_handler = logging.FileHandler(log_filepath, encoding="utf-8")

            file_formatter = logging.Formatter(
                "%(asctime)s - %(levelname)s - %(name)s - %(message)s",
                datefmt=ColoredFormatter.DATE_FORMAT,
            )
            file_handler.setFormatter(file_formatter)

            file_handler.setLevel(LOG_FILE_LEVEL)

            _logger.addHandler(file_handler)

        except Exception as e:
            sys.stderr.write(
                f"{TermColors.RED}错误: 初始化文件日志记录失败: {e}{TermColors.RESET}\n"
            )
            sys.stderr.flush()

    return _logger


def get_logger():
    if _logger is None:
        sys.stderr.write(
            f"{TermColors.YELLOW}警告: 日志记录器在显式初始化之前被访问。 "
            f"将使用默认值进行初始化。{TermColors.RESET}\n"
        )
        sys.stderr.flush()
        initialize_logger()
    return _logger


def log_debug(message, *args, **kwargs):
    get_logger().debug(message, *args, **kwargs)


def log_info(message, *args, **kwargs):
    get_logger().info(message, *args, **kwargs)


def log_warning(message, *args, **kwargs):
    get_logger().warning(message, *args, **kwargs)


def log_error(message, *args, **kwargs):
    get_logger().error(message, *args, **kwargs)


def log_info_color(message, color_code=TermColors.GREEN, *args, **kwargs):
    get_logger().info(f"{color_code}{message}{TermColors.RESET}", *args, **kwargs)


def log_warning_color(message, color_code=TermColors.YELLOW, *args, **kwargs):
    get_logger().warning(f"{color_code}{message}{TermColors.RESET}", *args, **kwargs)


def log_error_color(message, color_code=TermColors.RED, *args, **kwargs):
    get_logger().error(f"{color_code}{message}{TermColors.RESET}", *args, **kwargs)


def log_rag_output(message, *args, **kwargs):
    get_logger().info(f"{TermColors.BLUE}{message}{TermColors.RESET}", *args, **kwargs)


# --- 使用示例 ---
if __name__ == "__main__":
    # 1. 初始化日志记录器
    initialize_logger(app_name="演示应用", config_debug_mode=True, show_timestamp=True)
    log_info("=============== 炫彩日志与加载动画演示开始 ===============")
    log_debug("这是一个调试消息：日志系统已成功初始化。")
    if not ENABLE_FILE_LOGGING:
        log_warning("文件日志记录已禁用。如需启用，请设置 ENABLE_FILE_LOGGING = True")
    else:
        log_info(f"文件日志已启用，日志将存储在 '{LOG_FILE_DIRECTORY}' 目录下。")

    # 2. 基本日志级别演示
    log_info("演示2.1: log_info是一条 INFO 信息。")
    log_warning("演示2.2: log_warning是一条警告 WARNING 信息。")
    log_error("演示2.3: log_error是一条错误 ERROR 信息。")
    log_debug(
        "演示2.4: log_debug是一条调试 DEBUG 信息。DEBUG信息（包括对应时间戳）全部保持灰色"
    )
    log_info_color("演示2.5: log_info_color的 INFO 信息带有醒目的绿色。")
    log_warning_color("演示2.6: log_warning_color的 WARNING 信息带有醒目的黄色。")
    log_error_color("演示2.7: log_error_color的 ERROR 信息带有醒目的红色。")

    # 3. 加载动画演示
    log_info("演示3.1: 默认加载动画 (braille样式, 白色)")
    start_loading_animation(message="任务A处理中")
    time.sleep(2)
    stop_loading_animation(success=True, final_message="任务A成功完成!")

    log_info("演示3.2: 自定义动画样式 (spinner样式, 默认白色)")
    start_loading_animation(message="任务B执行中", animation_style_key="spinner")
    time.sleep(2)
    stop_loading_animation(success=True, final_message="任务B (spinner) 执行完毕!")

    log_info("演示3.3: 自定义动画颜色 (默认braille样式, 青色)")
    start_loading_animation(message="任务C加载中", animation_color=TermColors.CYAN)
    time.sleep(2)
    stop_loading_animation(success=True, final_message="任务C (青色) 加载完成!")

    log_info("演示3.4: 自定义样式与颜色 (arrows样式, 品红色)")
    start_loading_animation(
        message="任务D进行中",
        animation_style_key="arrows",
        animation_color=TermColors.MAGENTA,
    )
    time.sleep(2.5)
    stop_loading_animation(success=True, final_message="任务D (品红箭头) 完成!")

    log_info("演示3.5: 其他动画样式 (moon样式, 浅蓝色)")
    start_loading_animation(
        message="月相观察",
        animation_style_key="moon",
        animation_color=TermColors.LIGHT_BLUE,
    )
    time.sleep(2.5)
    stop_loading_animation(success=True, final_message="月相观察完毕!")

    log_info("演示3.6: 动画期间进行日志记录 (dots样式, 橙色)")
    start_loading_animation(
        message="橙色点点任务",
        animation_style_key="dots",
        animation_color=TermColors.ORANGE,
    )
    log_info("动画已启动，现在记录一条 INFO 消息，动画会自动避让。")
    time.sleep(1)
    log_warning("这是一条警告 WARNING 消息，动画仍在后台继续。")
    time.sleep(1)
    log_debug("一条调试 DEBUG 消息，动画即将停止并模拟失败。")
    time.sleep(1)
    stop_loading_animation(
        success=False,
        final_message="橙色点点任务模拟失败。使用success=False 会显示红叉",
    )

    log_info("演示3.7: 停止动画时不显示最终消息")
    start_loading_animation(message="短暂处理")
    time.sleep(1.5)
    stop_loading_animation()
    log_info(
        "动画已停止，不提供 final_message，则 stop_loading_animation 不输出额外消息。"
    )

    # 4. 特殊颜色日志函数
    log_info("演示4.1: 使用 log_info_color 输出自定义颜色 INFO (例如紫红色)")
    log_info_color("这是一条紫红色的 INFO 信息。", color_code=TermColors.MAGENTA)

    log_info("演示4.2: 使用 log_rag_output 输出特定格式 INFO 作为自定义log函数的示范")
    log_rag_output("这是一个使用log_rag_output输出的，模拟的 RAG 模型输出内容")

    # 5. 重新初始化日志记录器：关闭控制台时间戳
    log_info(
        "演示5: 重新初始化日志，关闭控制台时间戳 (文件日志不受影响)。重新初始化会基于当前时间创建新的日志文件（如果文件名是基于时间的）"
    )
    initialize_logger(
        app_name="演示应用-无时间戳", config_debug_mode=True, show_timestamp=False
    )
    log_info("这条 INFO 信息在控制台不显示时间戳。")
    log_debug("这条 DEBUG 信息在控制台也不显示时间戳。")
    start_loading_animation(message="无时间戳任务执行")
    time.sleep(1.5)
    stop_loading_animation(final_message="无时间戳任务完成。")
    log_info("控制台时间戳已关闭，文件日志中的时间戳格式依然由 file_formatter 控制。")

    # 6. 恢复时间戳并测试与 print() 的交互
    log_info("演示6: 恢复时间戳并测试动画与普通 print() 语句的交互")
    initialize_logger(
        app_name="演示应用", config_debug_mode=True, show_timestamp=True
    )  # 恢复默认配置
    log_info("日志时间戳已恢复。")

    print(
        f"{TermColors.YELLOW}这是一条普通的 print() 语句，在动画开始前。{TermColors.RESET}"
    )
    start_loading_animation(message="错误的动画与print交互写法")
    time.sleep(1)
    print(
        f"{TermColors.RED}错误示范: 下面这条 print() 语句会打断当前动画行，因为它直接写入stdout并通常会换行。只处理 logging 模块发出的日志，无法拦截 print()，不能正确关闭演示动画。{TermColors.RESET}"
    )
    time.sleep(1)
    log_info(
        "这条日志消息在 print() 之后，会由 AnimationAwareStreamHandler 正确处理，先清空动画行再输出。"
    )
    time.sleep(1)
    stop_loading_animation(final_message="动画与 print() 交互测试结束。")
    print(
        f"{TermColors.GREEN}动画结束后的另一条 print() 语句，可以正常显示。{TermColors.RESET}"
    )

    # 7. 结束
    if ENABLE_FILE_LOGGING:
        log_info(f"所有演示已完成。请检查 '{LOG_FILE_DIRECTORY}' 目录中的日志文件。")
    else:
        log_info("所有演示已完成。文件日志记录当前已禁用。")
    log_info("=============== 演示结束 ===============")
