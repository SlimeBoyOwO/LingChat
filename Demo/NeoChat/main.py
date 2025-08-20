# main.py
import os
import sys
from datetime import datetime

import config
from core.logger import initialize_logger, log_info, log_error, log_warning, log_debug, log_info_color, TermColors
from core.game_engine import GameEngine

def select_from_list(items, prompt):
    if not items:
        return None
    for i, item in enumerate(items):
        print(f"  [{i + 1}] {item}")
    while True:
        try:
            choice = input(f"{prompt} (输入数字): ")
            choice_idx = int(choice) - 1
            if 0 <= choice_idx < len(items):
                return items[choice_idx]
            else:
                print(f"{TermColors.RED}无效的数字，请输入 1 到 {len(items)} 之间的数字。{TermColors.RESET}")
        except ValueError:
            print(f"{TermColors.RED}无效的输入，请输入数字。{TermColors.RESET}")

def start_new_game():
    log_info("开始新游戏流程...")
    
    # 1. 选择剧本
    packs_path = config.STORY_PACKS_BASE_PATH
    available_packs = [d for d in os.listdir(packs_path) if os.path.isdir(os.path.join(packs_path, d))]
    if not available_packs:
        log_error(f"在 '{packs_path}' 目录下未找到任何剧本包。")
        return None
    
    print(f"\n{TermColors.YELLOW}请选择一个剧本包：{TermColors.RESET}")
    chosen_pack_name = select_from_list(available_packs, "选择剧本")
    if not chosen_pack_name: return None
    chosen_pack_path = os.path.join(packs_path, chosen_pack_name)

    # 2. 读取剧本配置，确定所需角色
    try:
        with open(os.path.join(chosen_pack_path, '全局剧情配置.yaml'), 'r', encoding='utf-8') as f:
            pack_config = yaml.safe_load(f)
        required_roles = pack_config['character_roles']
    except (FileNotFoundError, KeyError, yaml.YAMLError) as e:
        log_error(f"读取剧本 '{chosen_pack_name}' 的配置失败: {e}")
        return None
    
    # 3. 为每个角色选择人设
    chars_path = config.CHARACTERS_BASE_PATH
    available_chars = [f for f in os.listdir(chars_path) if f.endswith('.yaml')]
    if not available_chars:
        log_error(f"在 '{chars_path}' 目录下未找到任何人设文件。")
        return None
    
    character_selections = {}
    print(f"\n{TermColors.YELLOW}请为剧本中的每个角色选择一个人设：{TermColors.RESET}")
    for role_id in required_roles:
        print(f"--- 为角色 '{role_id}' 选择人设 ---")
        chosen_char_file = select_from_list(available_chars, f"选择人设")
        if not chosen_char_file: return None
        character_selections[role_id] = os.path.join(chars_path, chosen_char_file)
        # 从列表中移除已选的角色，避免重复选择
        available_chars.remove(chosen_char_file)

    # 4. (新增) 选择玩家人设
    player_data = None
    player_chars_path = config.PLAYER_CHARACTERS_BASE_PATH
    available_player_chars = [f for f in os.listdir(player_chars_path) if f.endswith('.yaml')]
    
    print(f"\n{TermColors.YELLOW}是否要导入自定义玩家人设？（否则将使用剧本默认设定）{TermColors.RESET}")
    if available_player_chars:
        # 在可选列表前增加"跳过"选项
        display_choices = ["[跳过] 使用剧本默认设定"] + available_player_chars
        chosen_player_char_file = select_from_list(display_choices, "选择玩家人设")

        # 如果玩家的选择不是"跳过"
        if chosen_player_char_file and chosen_player_char_file != display_choices[0]:
            try:
                player_char_path = os.path.join(player_chars_path, chosen_player_char_file)
                with open(player_char_path, 'r', encoding='utf-8') as f:
                    loaded_data = yaml.safe_load(f)
                    # 校验关键字段
                    if 'player_name' in loaded_data and 'player_prompt' in loaded_data:
                        player_data = loaded_data
                        log_info(f"成功加载玩家人设: {player_data['player_name']}")
                    else:
                        log_warning("选择的人设文件缺少 'player_name' 或 'player_prompt' 字段，将使用默认设定。")
            except (FileNotFoundError, yaml.YAMLError) as e:
                log_error(f"加载玩家人设文件失败: {e}，将使用默认设定。")
    else:
        log_info("未在 'player_characters' 目录中找到任何人设文件，将使用剧本默认设定。")

    # 5. (修改) 初始化游戏引擎
    engine = GameEngine()
    # 将加载到的 player_data 传递给引擎
    if engine.load_story_pack(chosen_pack_path, character_selections, player_data):
        return engine
    return None

def load_game_from_save():
    log_info("加载游戏存档...")
    saves_path = config.SAVES_BASE_PATH
    available_saves = [d for d in os.listdir(saves_path) if os.path.isdir(os.path.join(saves_path, d))]
    if not available_saves:
        log_warning("未找到任何存档。")
        return None

    print(f"\n{TermColors.YELLOW}请选择一个存档加载：{TermColors.RESET}")
    chosen_save_name = select_from_list(available_saves, "选择存档")
    if not chosen_save_name: return None
    
    engine = GameEngine()
    if engine.load_from_save(os.path.join(saves_path, chosen_save_name)):
        return engine
    return None


def game_loop(engine):
    log_info_color("游戏开始！在任何你需要输入的时候，都可以使用 /save 或 /load 命令。", TermColors.GREEN)
    
    # 初次运行
    engine.run()

    while engine.is_running and not engine.game_over:
        try:
            user_input = input()
        except (EOFError, KeyboardInterrupt):
            print("\n再见！")
            break

        if user_input.lower().startswith('/save'):
            parts = user_input.split(maxsplit=1)
            save_name = parts[1] if len(parts) > 1 else f"自动存档_{datetime.now().strftime('%H%M%S')}"
            engine.save_game(save_name)
            # 保存后继续等待当前输入
            if engine.progress['runtime_state'] == "WaitingForPlayerInput":
                print(f"{TermColors.YELLOW}你:{TermColors.RESET} ", end="")
            continue
        
        if user_input.lower().startswith('/load'):
            log_warning("在游戏中加载会丢失当前进度。此功能最好在主菜单使用。")
            # 在此简化demo中，游戏中加载会退出当前游戏循环
            return 'load' 

        engine.provide_player_input(user_input)

    log_info_color("游戏结束。", TermColors.MAGENTA)
    return 'menu'


def main():
    initialize_logger(config_debug_mode=config.DEBUG_MODE)
    
    if not config.API_KEY or "YOUR_DEEPSEEK_API_KEY" in config.API_KEY:
        log_error("请在 config.py 文件中设置你的 DeepSeek API Key。")
        return
        
    # 确保文件夹存在
    for path in [config.SAVES_BASE_PATH, config.STORY_PACKS_BASE_PATH, config.CHARACTERS_BASE_PATH, config.PLAYER_CHARACTERS_BASE_PATH]:
        os.makedirs(path, exist_ok=True)

    while True:
        print("\n" + "="*30)
        print(" NeoChat 0.4 - 命令行演示")
        print("="*30)
        print(f"{TermColors.CYAN}  /start - 开始新游戏")
        print(f"  /load  - 加载存档")
        print(f"  /exit  - 退出程序{TermColors.RESET}")
        
        command = input("> ").lower().strip()

        engine = None
        if command == '/start':
            engine = start_new_game()
        elif command == '/load':
            engine = load_game_from_save()
        elif command == '/exit':
            break
        else:
            log_warning("无效的命令。")
            continue
        
        if engine:
            result = game_loop(engine)
            if result == 'load':
                # 如果游戏中加载，则循环到主菜单重新加载
                continue

if __name__ == "__main__":
    import yaml # 确保PyYAML已安装
    main()
