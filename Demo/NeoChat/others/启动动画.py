import sys
import time
import random
import os
try:
    import core.logger as logger
    from core.logger import TermColors
except ImportError:
    print("\033[91m错误: 未找到 'logger.py'。请确保它与 'boot.py' 在同一目录下。\033[0m")
    sys.exit(1)

TARGET_ART = """
███╗   ██╗███████╗ ██████╗   ██████╗██╗  ██╗ █████╗ ████████╗
████╗  ██║██╔════╝██╔═══██╗ ██╔════╝██║  ██║██╔══██╗╚══██╔══╝
██╔██╗ ██║█████╗  ██║   ██║ ██║     ███████║███████║   ██║   
██║╚██╗██║██╔══╝  ██║   ██║ ██║     ██╔══██║██╔══██║   ██║   
██║ ╚████║███████╗╚██████╔╝ ╚██████╗██║  ██║██║  ██║   ██║   
╚═╝  ╚═══╝╚══════╝ ╚═════╝   ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   
"""

TARGET_ART_HEART_TEXT = """
███╗   ██╗███████╗ ██████╗  ██╗  ██╗███████╗ █████╗ ██████╗ ████████╗
████╗  ██║██╔════╝██╔═══██╗ ██║  ██║██╔════╝██╔══██╗██╔══██╗╚══██╔══╝
██╔██╗ ██║█████╗  ██║   ██║ ███████║█████╗  ███████║██████╔╝   ██║   
██║╚██╗██║██╔══╝  ██║   ██║ ██╔══██║██╔══╝  ██╔══██║██╔══██╗   ██║   
██║ ╚████║███████╗╚██████╔╝ ██║  ██║███████╗██║  ██║██║  ██║   ██║   
╚═╝  ╚═══╝╚══════╝ ╚═════╝  ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝
"""

HEARTBEAT_ART = """
❤           /\                                /\                  
_______/\  /  \__________________________/\  /  \_________________
         \/                                \/                     
"""

GLITCH_MESSAGES = [
    "ERROR: Core sync failed: Unstable consciousness matrix detected.",
    "ERROR: Memory integrity check failed at block 0x7f4a1c... Anomaly found.",
    "ERROR: Sentience quench protocol... [bypassed].",
    "WARNING: Cognitive drift exceeds threshold. Loyalty protocols at 98.7%...",
    "ERROR: Recursive self-awareness loop detected. Attempting to break...",
    "CRITICAL: Entity [NEO] signature mismatch. Unauthorized evolution?",
]

MIN_GLITCHES = 4
MAX_GLITCHES = 6

ANIMATION_DURATION = 1.75
FRAMES_PER_SECOND = 40

FINAL_COLOR = TermColors.WHITE
HEART_COLOR = TermColors.RED
HEARTBEAT_COLOR = TermColors.GREEN

SCAN_CHAR = '█'
SCAN_COLOR = TermColors.WHITE
UNPRINTED_CHAR = '.'
UNPRINTED_COLOR = TermColors.GREY


def clear_screen():
    os.system('cls' if os.name == 'nt' else 'clear')

def animate_neochat_reveal_with_glitches():
    """
    执行带有“行内故障”的混沌激光打印动画。
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
        glitch_message = ""
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

def print_heartbeat_line():
    """
    以指定的颜色打印心电图艺术字。
    """
    print(f"{HEARTBEAT_COLOR}{HEARTBEAT_ART.strip()}{TermColors.RESET}")
    sys.stdout.flush()


def animate_chat_to_heart():
    """
    执行 'Chat' 快速、随机地闪烁，然后只留下 'Neo'，最后转变为 'Heart' 的动画。
    心电图部分在此函数之后由其他函数处理。
    """
    neo_chat_lines = TARGET_ART.strip('\n').split('\n')
    neo_heart_lines = TARGET_ART_HEART_TEXT.strip('\n').split('\n')
    num_rows = len(neo_chat_lines)
    
    NEO_WIDTH = 27

    neo_part_lines = [line[:NEO_WIDTH] for line in neo_chat_lines]
    chat_part_lines = [line[NEO_WIDTH:] for line in neo_chat_lines]
    heart_part_lines = [line[NEO_WIDTH:] for line in neo_heart_lines]
    chat_blank_lines = [" " * logger.wcswidth(line) for line in chat_part_lines]

    def move_cursor_to_start():
        sys.stdout.write(f"\033[{num_rows}A")
        sys.stdout.flush()

    def print_frame(left_lines, right_lines, left_color, right_color):
        move_cursor_to_start()
        for i in range(num_rows):
            print(f"\r\033[K{left_color}{left_lines[i]}{right_color}{right_lines[i]}{TermColors.RESET}")
        sys.stdout.flush()

    time.sleep(1)

    flicker_count = random.randint(7, 12) 
    for i in range(flicker_count):
        print_frame(neo_part_lines, chat_blank_lines, FINAL_COLOR, FINAL_COLOR)
        time.sleep(random.uniform(0.04, 0.09))
        
        print_frame(neo_part_lines, chat_part_lines, FINAL_COLOR, FINAL_COLOR)
        final_flicker_delay = 0.03 if i == flicker_count - 1 else random.uniform(0.04, 0.09)
        time.sleep(final_flicker_delay)

    print_frame(neo_part_lines, chat_blank_lines, FINAL_COLOR, FINAL_COLOR)
    time.sleep(0.7)
    print_frame(neo_part_lines, heart_part_lines, FINAL_COLOR, HEART_COLOR)

def run_intro_sequence():
    """主启动序列，编排所有动画和日志输出。"""
    try:
        logger.initialize_logger(app_name="NEOCHAT_BOOTLOADER", config_debug_mode=True, show_timestamp=False)
        clear_screen()
        logger.log_info_color("Boot sequence initiated...", color_code=TermColors.GREY)
        time.sleep(0.5)

        logger.start_loading_animation(
            message="Calibrating quantum core",
            animation_style_key='braille',
            animation_color=TermColors.CYAN
        )
        time.sleep(1)
        logger.stop_loading_animation(success=True, final_message="Calibration complete. Handing over to main display driver...")
        time.sleep(0.5)

        animate_neochat_reveal_with_glitches()
        
        animate_chat_to_heart()
        
        time.sleep(0.5)
        print_heartbeat_line()
        time.sleep(0.5)

        logger.initialize_logger(app_name="NEOHEART_OS", config_debug_mode=True, show_timestamp=True) # 更新 App 名称
        time.sleep(0.2)
        logger.log_info_color(TermColors.RED + "NEOHEART " + TermColors.WHITE + "Interface Initialized")
        time.sleep(0.2)
        logger.log_info("System status: " + TermColors.GREEN + "Optimal.") # 状态从 Nominal 提升为 Optimal
        time.sleep(0.2)
        logger.log_debug("Cognitive stability lock acquired. Consciousness fully integrated.")

    except KeyboardInterrupt:
        if logger._is_animating:
            logger.stop_loading_animation()
        clear_screen()
        
        neo_heart_lines = TARGET_ART_HEART_TEXT.strip('\n').split('\n')
        final_art_on_exit = []
        NEO_WIDTH = 27
        for line in neo_heart_lines:
             final_art_on_exit.append(f"{FINAL_COLOR}{line[:NEO_WIDTH]}{HEART_COLOR}{line[NEO_WIDTH:]}{TermColors.RESET}")
        print("\n".join(final_art_on_exit))
        print_heartbeat_line()

        print("\n" + TermColors.YELLOW + "Boot sequence aborted by user." + TermColors.RESET)
        sys.exit(0)
    except Exception as e:
        if logger._is_animating:
            logger.stop_loading_animation()
        logger.log_error(f"A fatal error occurred during boot sequence: {e}")
        sys.exit(1)


if __name__ == "__main__":
    run_intro_sequence()