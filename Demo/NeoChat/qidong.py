import os
import time
import random
import sys
from colorama import init, Fore, Style

# 初始化 colorama
init(autoreset=True)

# ==========================================================
#                  可配置参数
# ==========================================================

TARGET_ART = """
███╗   ██╗███████╗ ██████╗  ██████╗██╗  ██╗ █████╗ ████████╗
████╗  ██║██╔════╝██╔═══██╗██╔════╝██║  ██║██╔══██╗╚══██╔══╝
██╔██╗ ██║█████╗  ██║   ██║██║     ███████║███████║   ██║   
██║╚██╗██║██╔══╝  ██║   ██║██║     ██╔══██║██╔══██║   ██║   
██║ ╚████║███████╗╚██████╔╝╚██████╗██║  ██║██║  ██║   ██║   
╚═╝  ╚═══╝╚══════╝ ╚═════╝  ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   
"""

# 动画总时长（秒） - 保持不变
ANIMATION_DURATION = 2.5

# 动画的流畅度（每秒的帧数）
FRAMES_PER_SECOND = 20

# 最终图案的颜色
FINAL_COLOR = Fore.CYAN

# “激光”光束的字符和颜色
SCAN_CHAR = '█'
SCAN_COLOR = Fore.WHITE + Style.BRIGHT

# 未打印区域的字符和颜色（低对比度）
UNPRINTED_CHAR = '.'
UNPRINTED_COLOR = Fore.BLACK + Style.BRIGHT # 在黑色背景下显示为深灰色

# ==========================================================

def clear_screen():
    """根据操作系统清空终端屏幕"""
    os.system('cls' if os.name == 'nt' else 'clear')

def animate_chaotic_laser():
    """执行混沌激光打印/随机汇聚动画"""
    # 1. 准备数据
    target_lines = TARGET_ART.strip('\n').split('\n')
    max_width = max(len(line) for line in target_lines) if target_lines else 0
    padded_lines = [line.ljust(max_width) for line in target_lines]

    # 获取所有非空白字符的坐标 (行, 列)，这些是需要被“打印”的点
    glyph_coords = []
    for r, line in enumerate(padded_lines):
        for c, char in enumerate(line):
            if not char.isspace():
                glyph_coords.append((r, c))

    # **核心混沌机制**：将所有待打印的坐标随机打乱
    random.shuffle(glyph_coords)

    # 2. 计算动画参数
    total_frames = int(ANIMATION_DURATION * FRAMES_PER_SECOND)
    frame_delay = 1.0 / FRAMES_PER_SECOND
    total_glyphs = len(glyph_coords)
    # 每帧需要处理（扫描或揭示）的像素点数量
    glyphs_per_frame = max(1, total_glyphs // total_frames)

    # 3. 动画状态变量
    revealed_coords = set()  # 已永久揭示的坐标
    last_frame_scanned = set() # 上一帧被扫描的坐标
    revealed_cursor = 0      # 指向 glyph_coords 列表的游标

    # 4. 开始动画循环
    for frame in range(total_frames + 2): # +2 确保最后一次扫描也能显示
        
        # 步骤 A: 将上一帧扫描过的点，转为永久揭示状态
        revealed_coords.update(last_frame_scanned)
        
        # 步骤 B: 确定本帧要新扫描的点
        start_index = revealed_cursor
        end_index = min(total_glyphs, start_index + glyphs_per_frame)
        scanning_coords = set(glyph_coords[start_index:end_index])
        revealed_cursor = end_index

        # 步骤 C: 将本帧扫描的点存起来，以便下一帧使用
        last_frame_scanned = scanning_coords
        
        clear_screen()
        
        # 5. 构建并打印当前帧
        output_buffer = []
        for r, line in enumerate(padded_lines):
            new_line = []
            for c, char in enumerate(line):
                coord = (r, c)
                # 状态 1: 已永久揭示
                if coord in revealed_coords:
                    new_line.append(FINAL_COLOR + char)
                # 状态 2: 正在被本帧的“激光”扫描
                elif coord in scanning_coords:
                    new_line.append(SCAN_COLOR + SCAN_CHAR)
                # 状态 3: 是图案的一部分，但还未被触及
                elif not char.isspace():
                    new_line.append(UNPRINTED_COLOR + UNPRINTED_CHAR)
                # 状态 4: 是背景空白区域
                else:
                    new_line.append(' ')
            output_buffer.append("".join(new_line))

        # 打印构建好的帧 (使用 RESET_ALL 避免颜色溢出)
        sys.stdout.write(Style.RESET_ALL + "\n".join(output_buffer) + "\n")
        sys.stdout.flush()

        # 如果所有点都已扫描完毕，提前结束循环
        if not scanning_coords and revealed_cursor >= total_glyphs:
            break
            
        time.sleep(frame_delay)
    
    # 6. 动画结束，显示最终的、清晰的图案
    clear_screen()
    final_output = FINAL_COLOR + TARGET_ART.strip('\n')
    print(final_output)
    print("\n" + Fore.GREEN + "NEOCHAT Initialized." + Style.RESET_ALL)

if __name__ == "__main__":
    try:
        animate_chaotic_laser()
    except KeyboardInterrupt:
        print("\n\nAnimation interrupted. Displaying final art.")
        clear_screen()
        final_output = FINAL_COLOR + TARGET_ART.strip('\n')
        print(final_output)
        sys.exit(0)