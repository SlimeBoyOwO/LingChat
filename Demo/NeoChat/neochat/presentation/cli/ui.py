# neochat/presentation/cli/ui.py
from typing import Dict, Optional

# TermColors 现在从 logging 模块导入
from neochat.platform.logging import TermColors

class ConsoleUI:
    """负责所有与控制台的输入输出。"""
    def display_narration(self, text: str):
        print(f"{TermColors.GREY}旁白: {text}{TermColors.RESET}")

    def display_dialogue(self, character_name: str, text: str):
        print(f"{TermColors.CYAN}{character_name}:{TermColors.RESET} {text}")

    def display_player_dialogue(self, player_name: str, text: str):
        print(f"{TermColors.YELLOW}{player_name}:{TermColors.RESET} {text}")

    def display_system_message(self, text: str, color: str = TermColors.BLUE):
        print(f"{color}{text}{TermColors.RESET}")

    def display_chapter(self, title: str, description: Optional[str] = None):
        print(f"\n{TermColors.GREEN}===== {title} ====={TermColors.RESET}")
        if description:
            print(f"{TermColors.GREY}{description}{TermColors.RESET}\n")

    def display_choices(self, choices: Dict[str, str]):
        """显示玩家选项。 e.g., {'A': '攻击', 'B': '逃跑'}"""
        self.display_system_message("请做出你的选择：", color=TermColors.YELLOW)
        for key, text in choices.items():
            print(f"  [{key}] {text}")

    def prompt_for_input(self, prompt_text: Optional[str] = None) -> str:
        """获取玩家输入。"""
        if prompt_text:
            print(f"{TermColors.YELLOW}你 (可输入或直接回车使用默认): {prompt_text}{TermColors.RESET}")
        else:
            print(f"{TermColors.YELLOW}你:{TermColors.RESET} ", end="")
        
        # 捕获 Ctrl+C/Ctrl+D 中断
        try:
            return input()
        except (KeyboardInterrupt, EOFError):
            print("\n操作已取消。")
            return "/exit" # 返回一个可以被主循环处理的命令

    def clear_screen(self):
        # 简单的清屏实现，在主菜单循环前调用
        print("\n" * 3) # 打印几个换行符来分隔内容