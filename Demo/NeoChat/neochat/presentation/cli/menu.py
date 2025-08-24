# neochat/presentation/cli/menu.py
import os
import sys
import yaml
import json # <--- 新增导入 json 模块

# 更新 import 路径
from neochat.platform.configuration import config
from neochat.platform.persistence.save_manager import SaveManager
from neochat.presentation.cli.ui import ConsoleUI
from neochat.presentation.cli.wizard import SetupWizard
from neochat.game.engine import GameEngine
from neochat.platform.logging import log_info, log_error, log_warning, TermColors

class MainMenu:
    """处理主菜单交互、游戏启动和加载的类。"""
    def __init__(self):
        self.ui = ConsoleUI()
        self.save_manager = SaveManager()

    def run(self):
        """启动主菜单循环。"""
        # 4. 处理首次启动的用户设置
        if SetupWizard.is_first_launch():
            log_info("检测到首次启动或用户配置缺失，将运行初始化向导。")
            wizard = SetupWizard(self.ui)
            wizard.run()
            log_info("初始化完成。请重新启动程序以应用新的用户配置并开始游戏。")
            self.ui.prompt_for_input("按回车键退出...")
            return

        # 5. 主菜单循环
        while True:
            self.ui.clear_screen()
            print("\n" + "="*30)
            print(" NeoChat-alpha")
            print("="*30)
            print(f"{TermColors.CYAN}  /start - 开始新游戏")
            print(f"  /load  - 加载存档")
            print(f"  /exit  - 退出程序{TermColors.RESET}")

            command = self.ui.prompt_for_input("> ").lower().strip()

            engine = None
            if command == '/start':
                engine = self._start_new_game()
            elif command == '/load':
                engine = self._load_game_from_save()
            elif command == '/exit':
                print("\n再见！")
                break
            else:
                self.ui.display_system_message("无效的命令。", TermColors.RED)
                self.ui.prompt_for_input("按回车键继续...")
                continue

            if engine:
                engine.run()
                self.ui.prompt_for_input("游戏结束。按回车键返回主菜单...")

    def _select_from_list(self, items: list, prompt: str) -> any:
        # ... (此方法不变) ...
        if not items:
            self.ui.display_system_message("没有可选项。", TermColors.RED)
            return None
        for i, item in enumerate(items):
            print(f"  [{i + 1}] {item}")
        while True:
            try:
                choice_str = self.ui.prompt_for_input(f"{prompt} (输入数字)")
                if choice_str == "/exit": return None # 允许在选择时退出
                choice_idx = int(choice_str) - 1
                if 0 <= choice_idx < len(items):
                    return items[choice_idx]
                else:
                    self.ui.display_system_message(f"无效的数字，请输入 1 到 {len(items)} 之间的数字。", TermColors.RED)
            except ValueError:
                self.ui.display_system_message("无效的输入，请输入一个数字。", TermColors.RED)
            except (EOFError, KeyboardInterrupt):
                self.ui.display_system_message("\n操作取消。", TermColors.YELLOW)
                return None


    # <--- 新增辅助函数来加载用户配置中的名字
    def _get_user_name_from_config(self) -> str:
        """尝试从 user_config.json 加载用户姓名，如果失败则返回默认值 '玩家'。"""
        try:
            user_config_path = config.paths.user_config
            if os.path.exists(user_config_path):
                with open(user_config_path, 'r', encoding='utf-8') as f:
                    user_data = json.load(f)
                return user_data.get("user_name", "玩家")
        except (IOError, json.JSONDecodeError) as e:
            log_warning(f"加载用户配置失败: {e}，将使用默认玩家名称。")
        return "玩家"
    # 新增辅助函数结束 --->

    def _start_new_game(self) -> GameEngine | None:
        """处理创建新游戏的完整流程。"""
        log_info("开始新游戏流程...")

        # ... (选择剧本代码不变) ...

        # 1. 选择剧本
        packs_path = config.paths.story_packs
        available_packs = [d for d in os.listdir(packs_path) if os.path.isdir(os.path.join(packs_path, d))]
        if not available_packs:
            log_error(f"在 '{packs_path}' 目录下未找到任何剧本包。")
            return None

        self.ui.display_system_message("\n请选择一个剧本包：", TermColors.YELLOW)
        chosen_pack_name = self._select_from_list(available_packs, "选择剧本")
        if not chosen_pack_name: return None
        chosen_pack_path = os.path.join(packs_path, chosen_pack_name)

        # 2. 读取剧本配置
        pack_config = self.save_manager.load_story_pack_config(chosen_pack_path)
        if not pack_config:
            log_error(f"无法加载剧本 '{chosen_pack_name}' 的配置。")
            return None

        # 3. 为AI角色选择人设
        chars_path = config.paths.characters
        available_chars_files = [f for f in os.listdir(chars_path) if f.endswith('.yaml')]
        if not available_chars_files:
            log_error(f"在 '{chars_path}' 目录下未找到任何人设文件。")
            return None

        character_selections = {}
        current_available_chars = list(available_chars_files)

        self.ui.display_system_message("\n请为剧本中的每个AI角色选择一个人设：", TermColors.YELLOW)
        for role_id in pack_config.character_roles:
            self.ui.display_system_message(f"--- 为角色 '{role_id}' 选择人设 ---", color=TermColors.MAGENTA)
            if not current_available_chars:
                log_error(f"可用的人设文件不足以分配给所有角色。")
                return None
            
            chosen_char_file = self._select_from_list(current_available_chars, f"为 '{role_id}' 选择人设")
            if not chosen_char_file: return None
            character_selections[role_id] = os.path.join(chars_path, chosen_char_file)
            current_available_chars.remove(chosen_char_file)

        # 4. 选择玩家人设
        # <--- 从这里开始修改 --->
        default_player_name = self._get_user_name_from_config() # <-- 在这里获取用户配置中的名字
        player_data = {'player_name': default_player_name, 'player_prompt': ''} # <-- 使用获取到的名字作为默认值

        player_chars_path = config.paths.player_characters
        available_player_chars_files = [f for f in os.listdir(player_chars_path) if f.endswith('.yaml')]

        self.ui.display_system_message("\n是否要导入自定义玩家人设？（否则将使用默认设定）", TermColors.YELLOW)
        if available_player_chars_files:
            display_choices = ["[跳过] 使用默认玩家设定"] + available_player_chars_files
            chosen_player_char_file = self._select_from_list(display_choices, "选择玩家人设")

            if chosen_player_char_file and chosen_player_char_file != display_choices[0]:
                try:
                    player_char_path = os.path.join(player_chars_path, chosen_player_char_file)
                    with open(player_char_path, 'r', encoding='utf-8') as f:
                        loaded_data = yaml.safe_load(f)
                        # 仅当文件中明确定义了 player_name 时才覆盖，否则沿用 user_config.json 的名字
                        player_data['player_name'] = loaded_data.get('player_name', player_data['player_name'])
                        player_data['player_prompt'] = loaded_data.get('player_prompt', '')
                        log_info(f"成功加载玩家人设: {player_data['player_name']}")
                except (FileNotFoundError, yaml.YAMLError) as e:
                    log_error(f"加载玩家人设文件失败: {e}，将使用默认设定。")
        else:
            log_info("未在 'player_characters' 目录中找到任何人设文件，将使用默认设定。")
        # <--- 修改结束 --->

        # 5. 创建游戏会话
        session = self.save_manager.create_new_game_session(chosen_pack_path, character_selections, player_data)
        if session:
            return GameEngine(session)
        return None

    def _load_game_from_save(self) -> GameEngine | None:
        # ... (此方法不变，因为加载存档时玩家名字会从存档文件中的 game_state 读取) ...
        log_info("加载游戏存档...")
        saves_path = config.paths.saves
        available_saves = [d for d in os.listdir(saves_path) if os.path.isdir(os.path.join(saves_path, d))]
        if not available_saves:
            log_warning("未找到任何存档。")
            return None

        self.ui.display_system_message("\n请选择一个存档加载：", TermColors.YELLOW)
        chosen_save_name = self._select_from_list(available_saves, "选择存档")
        if not chosen_save_name: return None

        session = self.save_manager.load_game_session(os.path.join(saves_path, chosen_save_name))
        if session:
            return GameEngine(session)
        return None