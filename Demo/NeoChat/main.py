import os
import sys
import yaml

sys.path.append(os.path.dirname(os.path.abspath(__file__)))

import config
from core.logger import initialize_logger, log_info, log_error, log_warning, TermColors
from core.engine import GameEngine
from core.save_manager import SaveManager
from core.ui import ConsoleUI
from core.setup_wizard import SetupWizard
from core.boot_animation import run_boot_animation

def select_from_list(items: list, prompt: str, ui: ConsoleUI) -> any:
    if not items:
        ui.display_system_message("没有可选项。", TermColors.RED)
        return None
    for i, item in enumerate(items):
        print(f"  [{i + 1}] {item}")
    while True:
        try:
            choice_str = ui.prompt_for_input(f"{prompt} (输入数字)")
            if choice_str is None:
                return None
            choice_idx = int(choice_str) - 1
            if 0 <= choice_idx < len(items):
                return items[choice_idx]
            else:
                ui.display_system_message(f"无效的数字，请输入 1 到 {len(items)} 之间的数字。", TermColors.RED)
        except ValueError:
            ui.display_system_message("无效的输入，请输入一个数字。", TermColors.RED)
        except (EOFError, KeyboardInterrupt):
            ui.display_system_message("\n操作取消。", TermColors.YELLOW)
            return None

def start_new_game(save_manager: SaveManager, ui: ConsoleUI) -> GameEngine | None:
    log_info("开始新游戏流程...")
    
    # 1. 选择剧本
    packs_path = config.STORY_PACKS_BASE_PATH
    available_packs = [d for d in os.listdir(packs_path) if os.path.isdir(os.path.join(packs_path, d))]
    if not available_packs:
        log_error(f"在 '{packs_path}' 目录下未找到任何剧本包。请确保至少有一个剧本包。")
        return None
    
    ui.display_system_message("\n请选择一个剧本包：", TermColors.YELLOW)
    chosen_pack_name = select_from_list(available_packs, "选择剧本", ui)
    if not chosen_pack_name: return None
    chosen_pack_path = os.path.join(packs_path, chosen_pack_name)

    # 2. 读取剧本配置，确定所需角色
    pack_config = save_manager.load_story_pack_config(chosen_pack_path)
    if not pack_config: 
        log_error(f"无法加载剧本 '{chosen_pack_name}' 的配置。")
        return None

    # 3. 为每个AI角色选择人设
    chars_path = config.CHARACTERS_BASE_PATH
    available_chars_files = [f for f in os.listdir(chars_path) if f.endswith('.yaml')]
    if not available_chars_files:
        log_error(f"在 '{chars_path}' 目录下未找到任何人设文件。请至少创建一个AI角色人设。")
        return None
    
    character_selections = {}
    # 复制一份可用角色列表，以便在选择后移除，避免重复选择
    current_available_chars = list(available_chars_files) 

    ui.display_system_message("\n请为剧本中的每个AI角色选择一个人设：", TermColors.YELLOW)
    for role_id in pack_config.character_roles:
        ui.display_system_message(f"--- 为角色 '{role_id}' 选择人设 ---")
        if not current_available_chars:
            log_error(f"可用的人设文件不足以分配给所有角色。缺少 '{role_id}' 的人设。")
            return None
        
        chosen_char_file = select_from_list(current_available_chars, f"选择 '{role_id}' 的人设", ui)
        if not chosen_char_file: return None
        character_selections[role_id] = os.path.join(chars_path, chosen_char_file)
        current_available_chars.remove(chosen_char_file)

    # 4. 选择玩家人设
    player_data = {'player_name': '玩家', 'player_prompt': ''} 
    player_chars_path = config.PLAYER_CHARACTERS_BASE_PATH
    available_player_chars_files = [f for f in os.listdir(player_chars_path) if f.endswith('.yaml')]
    
    ui.display_system_message("\n是否要导入自定义玩家人设？（否则将使用默认设定）", TermColors.YELLOW)
    if available_player_chars_files:
        display_choices = ["[跳过] 使用默认玩家设定"] + available_player_chars_files
        chosen_player_char_file = select_from_list(display_choices, "选择玩家人设", ui)

        if chosen_player_char_file and chosen_player_char_file != display_choices[0]:
            try:
                player_char_path = os.path.join(player_chars_path, chosen_player_char_file)
                with open(player_char_path, 'r', encoding='utf-8') as f:
                    loaded_data = yaml.safe_load(f)
                    if 'player_name' in loaded_data and 'player_prompt' in loaded_data:
                        player_data = loaded_data
                        log_info(f"成功加载玩家人设: {player_data['player_name']}")
                    else:
                        log_warning("选择的人设文件缺少 'player_name' 或 'player_prompt' 字段，将使用默认设定。")
            except (FileNotFoundError, yaml.YAMLError) as e:
                log_error(f"加载玩家人设文件失败: {e}，将使用默认设定。")
    else:
        log_info("未在 'player_characters' 目录中找到任何人设文件，将使用默认设定。")

    # 5. 创建游戏会话并初始化引擎
    session = save_manager.create_new_game_session(chosen_pack_path, character_selections, player_data)
    if session:
        return GameEngine(session)
    return None


def load_game_from_save(save_manager: SaveManager, ui: ConsoleUI) -> GameEngine | None:
    log_info("加载游戏存档...")
    saves_path = config.SAVES_BASE_PATH
    available_saves = [d for d in os.listdir(saves_path) if os.path.isdir(os.path.join(saves_path, d))]
    if not available_saves:
        log_warning("未找到任何存档。")
        return None

    ui.display_system_message("\n请选择一个存档加载：", TermColors.YELLOW)
    chosen_save_name = select_from_list(available_saves, "选择存档", ui)
    if not chosen_save_name: return None
    
    session = save_manager.load_game_session(os.path.join(saves_path, chosen_save_name))
    if session:
        return GameEngine(session)
    return None

def main():
    # 0. 运行启动动画
    run_boot_animation()
    
    # 1. 初始化日志系统
    initialize_logger(config_debug_mode=config.DEBUG_MODE)
    ui = ConsoleUI()

    # 2. 检查API Key配置
    if not config.API_KEY or "YOUR_DEEPSEEK_API_KEY" in config.API_KEY:
        log_error("请在项目根目录的 '.env' 文件中设置你的 DeepSeek API Key (API_KEY)。")
        log_error("例如: API_KEY=\"sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"")
        ui.prompt_for_input("按回车键退出...") 
        return

    # 3. 确保核心文件夹存在
    for path in [config.SAVES_BASE_PATH, config.STORY_PACKS_BASE_PATH, config.CHARACTERS_BASE_PATH, config.PLAYER_CHARACTERS_BASE_PATH]:
        os.makedirs(path, exist_ok=True)
    os.makedirs('data', exist_ok=True)
    with open(os.path.join('data', '__init__.py'), 'w') as f: # 创建空的 __init__.py
        pass

    # 4. 处理首次启动的用户设置
    if SetupWizard.is_first_launch():
        log_info("检测到首次启动或用户配置缺失，将运行初始化向导。")
        wizard = SetupWizard(ui)
        wizard.run()
        log_info("初始化完成。请重新启动程序以应用新的用户配置并开始游戏。")
        ui.prompt_for_input("按回车键退出...")
        return

    # 5. 初始化 SaveManager (在首次启动后才需要)
    save_manager = SaveManager()

    # 6. 主菜单循环
    while True:
        ui.clear_screen()
        print("\n" + "="*30)
        print(" NeoChat-alpha")
        print("="*30)
        print(f"{TermColors.CYAN}  /start - 开始新游戏")
        print(f"  /load  - 加载存档")
        print(f"  /exit  - 退出程序{TermColors.RESET}")
        
        try:
            command = ui.prompt_for_input("> ").lower().strip()
        except (EOFError, KeyboardInterrupt):
            command = '/exit'

        engine = None
        if command == '/start':
            engine = start_new_game(save_manager, ui)
        elif command == '/load':
            engine = load_game_from_save(save_manager, ui)
        elif command == '/exit':
            print("\n再见！")
            break
        else:
            ui.display_system_message("无效的命令。", TermColors.RED)
            ui.prompt_for_input("按回车键继续...")
            continue
        
        if engine:
            engine.run()
            ui.prompt_for_input("游戏结束。按回车键返回主菜单...")

if __name__ == "__main__":
    main()