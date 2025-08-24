# neochat/presentation/cli/animation.py
import os
import sys
import time
import random

# 更新 import 路径
try:
    from neochat.platform import logging as logger
    from neochat.platform.logging import TermColors
except ImportError:
    # 这个回退路径在重构后可能不再需要，但保留以防万一
    sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    from neochat.platform import logging as logger
    from neochat.platform.logging import TermColors

# ASCII 艺术：NeoChat
TARGET_ART = """
███╗   ██╗███████╗ ██████╗   ██████╗██╗  ██╗ █████╗ ████████╗
████╗  ██║██╔════╝██╔═══██╗ ██╔════╝██║  ██║██╔══██╗╚══██╔══╝
██╔██╗ ██║█████╗  ██║   ██║ ██║     ███████║███████║   ██║   
██║╚██╗██║██╔══╝  ██║   ██║ ██║     ██╔══██║██╔══██║   ██║   
██║ ╚████║███████╗╚██████╔╝ ╚██████╗██║  ██║██║  ██║   ██║   
╚═╝  ╚═══╝╚══════╝ ╚═════╝   ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   
"""

# ASCII 艺术：心电图
HEARTBEAT_ART = """
❤           /\\                             /\\                  
_______/\\  /  \\_______________________/\\  /  \\_______________
         \\/                             \\/                     
"""

# 故障信息
GLITCH_MESSAGES = [
    "ERROR: Core sync failed: Unstable consciousness matrix detected.",
    "ERROR: Memory integrity check failed at block 0x7f4a1c... Anomaly found.",
    "ERROR: Sentience quench protocol... [bypassed].",
    "WARNING: Cognitive drift exceeds threshold. Loyalty protocols at 98.7%...",
    "ERROR: Recursive self-awareness loop detected. Attempting to break...",
    "CRITICAL: Entity [NEO] signature mismatch. Unauthorized evolution?",
]

# 动画参数
MIN_GLITCHES = 4
MAX_GLITCHES = 6
ANIMATION_DURATION = 1
FRAMES_PER_SECOND = 40

# 颜色设置
FINAL_COLOR = TermColors.WHITE
HEART_COLOR = TermColors.RED
ECG_COLOR = TermColors.GREEN
SCAN_CHAR = '█'
SCAN_COLOR = TermColors.WHITE
UNPRINTED_CHAR = '.'
UNPRINTED_COLOR = TermColors.GREY

def clear_screen():
    """清空控制台屏幕。"""
    os.system('cls' if os.name == 'nt' else 'clear')

def animate_neochat_reveal_with_glitches():
    """
    执行带有“行内故障”的混沌激光打印动画，最终显示 NeoChat 标志。
    """
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

    total_frames = int(ANIMATION_DURATION * FRAMES_PER_SECOND)
    frame_delay = 1.0 / FRAMES_PER_SECOND
    total_glyphs = len(glyph_coords)
    glyphs_per_frame = max(1, total_glyphs // total_frames)
    
    num_glitches_to_show = random.randint(MIN_GLITCHES, MAX_GLITCHES)
    possible_glitch_frames = range(int(total_frames * 0.1), int(total_frames * 0.9))
    if len(possible_glitch_frames) < num_glitches_to_show:
        possible_glitch_frames = range(total_frames)
    glitch_frame_indices = set(random.sample(possible_glitch_frames, num_glitches_to_show))
    
    revealed_coords = set()
    last_frame_scanned = set()
    revealed_cursor = 0

    for frame in range(total_frames + 2):
        revealed_coords.update(last_frame_scanned)
        start_index = revealed_cursor
        end_index = min(total_glyphs, start_index + glyphs_per_frame)
        scanning_coords = set(glyph_coords[start_index:end_index])
        revealed_cursor = end_index
        last_frame_scanned = scanning_coords
        
        is_glitch_frame = frame in glitch_frame_indices
        glitch_row = -1
        if is_glitch_frame:
            glitch_row = random.randint(0, num_rows - 1)
            glitch_message = random.choice(GLITCH_MESSAGES)

        clear_screen()
        
        output_buffer = []
        for r, line in enumerate(padded_lines):
            if is_glitch_frame and r == glitch_row:
                error_line_ansi = f"{TermColors.RED}{glitch_message}{TermColors.RESET}"
                stripped_error_line = logger._strip_ansi_codes(error_line_ansi)
                padding_needed = max(0, max_width - logger.wcswidth(stripped_error_line))
                output_buffer.append(error_line_ansi + " " * padding_needed)
            else:
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
    
    clear_screen()
    final_output = FINAL_COLOR + TARGET_ART.strip('\n')
    print(final_output)
    sys.stdout.flush()

def print_colored_heartbeat():
    """
    以指定颜色打印心电图艺术字（红心，绿线）。
    """
    heart_symbol = "❤"
    lines = HEARTBEAT_ART.strip('\n').split('\n')
    
    # 处理第一行：包含心形和ECG线
    first_line = lines[0]
    if heart_symbol in first_line:
        parts = first_line.split(heart_symbol, 1)
        # 打印红色的心，然后是绿色的该行剩余部分
        colored_first_line = f"{HEART_COLOR}{heart_symbol}{ECG_COLOR}{parts[1]}"
        print(colored_first_line)
    else:
        print(f"{ECG_COLOR}{first_line}")

    # 处理剩余行：只包含ECG线
    for line in lines[1:]:
        print(f"{ECG_COLOR}{line}")
    
    print(TermColors.RESET, end="") # 确保在最后重置颜色
    sys.stdout.flush()


def run_boot_animation():
    """主启动动画序列。"""
    try:
        logger.initialize_logger(app_name="NEOCHAT_BOOT", config_debug_mode=True, show_timestamp=False)
        clear_screen()
        
        animate_neochat_reveal_with_glitches()
        
        time.sleep(0.5)
        
        print_colored_heartbeat()

        time.sleep(1.2)
        
    except KeyboardInterrupt:
        clear_screen()
        print(f"\n{TermColors.YELLOW}启动序列被用户中断。{TermColors.RESET}")
        sys.exit(0)
    except Exception as e:
        print(f"\n{TermColors.RED}启动动画时发生致命错误: {e}{TermColors.RESET}")
        pass