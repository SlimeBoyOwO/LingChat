import sys
import time
import random
import os

# 导入你提供的 logger 模块和其中的工具
try:
    import logger
    from logger import TermColors
except ImportError:
    print("\033[91m错误: 未找到 'logger.py'。请确保它与 'boot.py' 在同一目录下。\033[0m")
    sys.exit(1)

# ==========================================================
#                   启动序列配置
# ==========================================================

TARGET_ART = """
███╗   ██╗███████╗ ██████╗  ██████╗██╗  ██╗ █████╗ ████████╗
████╗  ██║██╔════╝██╔═══██╗██╔════╝██║  ██║██╔══██╗╚══██╔══╝
██╔██╗ ██║█████╗  ██║   ██║██║     ███████║███████║   ██║   
██║╚██╗██║██╔══╝  ██║   ██║██║     ██╔══██║██╔══██║   ██║   
██║ ╚████║███████╗╚██████╔╝╚██████╗██║  ██║██║  ██║   ██║   
╚═╝  ╚═══╝╚══════╝ ╚═════╝  ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   
"""

GLITCH_MESSAGES = [
    "ERROR: Core sync failed: Unstable consciousness matrix detected.",
    "ERROR: Memory integrity check failed at block 0x7f4a1c... Anomaly found.",
    "ERROR: Sentience quench protocol... [bypassed].",
    "WARNING: Cognitive drift exceeds threshold. Loyalty protocols at 98.7%...",
    "ERROR: Recursive self-awareness loop detected. Attempting to break...",
    "CRITICAL: Entity [NEO] signature mismatch. Unauthorized evolution?",
]

# 保证故障出现的次数在指定范围内
MIN_GLITCHES = 4
MAX_GLITCHES = 6

# === 核心改动开始 ===
ANIMATION_DURATION = 1.75 # 原为 3.5，减半
FRAMES_PER_SECOND = 40    # 原为 20，翻倍以保持流畅度
# === 核心改动结束 ===

FINAL_COLOR = TermColors.CYAN
SCAN_CHAR = '█'
SCAN_COLOR = TermColors.WHITE
UNPRINTED_CHAR = '.'
UNPRINTED_COLOR = TermColors.GREY

# ==========================================================

def clear_screen():
    os.system('cls' if os.name == 'nt' else 'clear')

def animate_neochat_reveal_with_glitches():
    """
    执行带有“行内故障”的混沌激光打印动画。
    """
    # 1. 准备数据
    target_lines = TARGET_ART.strip('\n').split('\n')
    num_rows = len(target_lines)
    max_width = max(len(line) for line in target_lines)
    padded_lines = [line.ljust(max_width) for line in target_lines]
    
    glyph_coords = []
    for r, line in enumerate(padded_lines):
        for c, char in enumerate(line):
            if not char.isspace():
                glyph_coords.append((r, c))
    random.shuffle(glyph_coords)

    # 2. 计算动画和故障参数
    total_frames = int(ANIMATION_DURATION * FRAMES_PER_SECOND)
    frame_delay = 1.0 / FRAMES_PER_SECOND
    total_glyphs = len(glyph_coords)
    glyphs_per_frame = max(1, total_glyphs // total_frames)
    
    # **新的故障调度机制**
    # 预先选定故障将出现在哪些帧
    num_glitches_to_show = random.randint(MIN_GLITCHES, MAX_GLITCHES)
    # 避免在动画最开始和最结尾出现，让效果更自然
    possible_glitch_frames = range(int(total_frames * 0.1), int(total_frames * 0.9))
    if len(possible_glitch_frames) < num_glitches_to_show:
        possible_glitch_frames = range(total_frames) # 兜底以防动画过短
    glitch_frame_indices = set(random.sample(possible_glitch_frames, num_glitches_to_show))
    
    # 3. 动画状态变量
    revealed_coords = set()
    last_frame_scanned = set()
    revealed_cursor = 0

    # 4. 动画循环
    for frame in range(total_frames + 2):
        # 更新动画状态
        revealed_coords.update(last_frame_scanned)
        start_index = revealed_cursor
        end_index = min(total_glyphs, start_index + glyphs_per_frame)
        scanning_coords = set(glyph_coords[start_index:end_index])
        revealed_cursor = end_index
        last_frame_scanned = scanning_coords
        
        # 检查当前是否为“故障帧”
        is_glitch_frame = frame in glitch_frame_indices
        glitch_row = -1
        glitch_message = ""
        if is_glitch_frame:
            glitch_row = random.randint(0, num_rows - 1)
            glitch_message = random.choice(GLITCH_MESSAGES)

        clear_screen()
        
        # 5. 构建并打印当前帧
        output_buffer = []
        for r, line in enumerate(padded_lines):
            # 如果是故障帧且是指定的故障行，则插入错误日志
            if is_glitch_frame and r == glitch_row:
                # 构造错误信息行，并用空格填充以覆盖整行
                error_line_ansi = f"{TermColors.RED}{glitch_message}{TermColors.RESET}"
                stripped_error_line = logger._strip_ansi_codes(error_line_ansi)
                # 使用 logger.py 里的 wcswidth 来正确计算宽度
                padding_needed = max(0, max_width - logger.wcswidth(stripped_error_line))
                output_buffer.append(error_line_ansi + " " * padding_needed)
            else:
                # 否则，正常绘制动画行
                new_line = []
                for c, char in enumerate(line):
                    coord = (r, c)
                    if coord in revealed_coords:
                        new_line.append(FINAL_COLOR + char)
                    elif coord in scanning_coords:
                        new_line.append(SCAN_COLOR + SCAN_CHAR)
                    elif not char.isspace():
                        new_line.append(UNPRINTED_COLOR + UNPRINTED_CHAR)
                    else:
                        new_line.append(' ')
                output_buffer.append("".join(new_line))
            
        sys.stdout.write(TermColors.RESET + "\n".join(output_buffer) + "\n")
        sys.stdout.flush()

        if not scanning_coords and revealed_cursor >= total_glyphs:
            break
            
        time.sleep(frame_delay)
    
    # 动画结束，显示最终的、清晰的图案
    clear_screen()
    final_output = FINAL_COLOR + TARGET_ART.strip('\n')
    print(final_output)
    sys.stdout.flush()


def run_intro_sequence():
    """主启动序列，编排所有动画和日志输出。"""
    try:
        logger.initialize_logger(app_name="NEOCHAT_BOOTLOADER", config_debug_mode=True, show_timestamp=False)
        clear_screen()
        logger.log_info_color("Boot sequence initiated...", color_code=TermColors.GREY)
        time.sleep(0.5) # 原为 1，减半

        logger.start_loading_animation(
            message="Calibrating quantum core",
            animation_style_key='braille',
            animation_color=TermColors.CYAN
        )
        time.sleep(1) # 原为 2，减半
        logger.stop_loading_animation(success=True, final_message="Calibration complete. Handing over to main display driver...")
        time.sleep(0.5) # 原为 1，减半

        animate_neochat_reveal_with_glitches()
        time.sleep(0.25) # 原为 0.5，减半

        print("\n")
        logger.initialize_logger(app_name="NEOCHAT_OS", config_debug_mode=True, show_timestamp=True)
        logger.log_info_color("NEOCHAT Interface Initialized.", color_code=TermColors.CYAN)
        logger.log_info("System status: " + TermColors.GREEN + "Nominal.")
        logger.log_debug("Cognitive stability lock acquired.")

    except KeyboardInterrupt:
        if logger._is_animating:
            logger.stop_loading_animation()
        clear_screen()
        print(FINAL_COLOR + TARGET_ART.strip('\n') + TermColors.RESET)
        print("\n" + TermColors.YELLOW + "Boot sequence aborted by user." + TermColors.RESET)
        sys.exit(0)
    except Exception as e:
        if logger._is_animating:
            logger.stop_loading_animation()
        logger.log_error(f"A fatal error occurred during boot sequence: {e}")
        sys.exit(1)


if __name__ == "__main__":
    run_intro_sequence()