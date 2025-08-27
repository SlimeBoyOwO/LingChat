import shutil
from pathlib import Path
from typing import List, Dict, Optional

from nbformat import current_nbformat
from sqlalchemy import null

from ling_chat.utils.function import Function
from ling_chat.core.ai_service.script_engine.type import Character, Script, Charpter
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import static_path, user_data_path
from ling_chat.core.ai_service.script_engine.charpter_manager import CharpterManager
from ling_chat.core.ai_service.script_engine.events_handler import EventsHandler
from ling_chat.core.ai_service.script_engine.ends_handler import EndsHandler
from ling_chat.core.ai_service.script_engine.exceptions import ScriptLoadError

class ScriptManager:
    def __init__(self):
        # 全局设定，确定剧本状态
        self.scripts_dir = user_data_path / "game_data" / "scripts"

        # 全部剧本管理
        self.all_scripts:list[str] = []
        self.get_all_scripts()

        # 辅助模块
        self.charpter_manager = CharpterManager()
        self.events_handler:EventsHandler = EventsHandler()
        self.ends_handler:EndsHandler = EndsHandler()

        # 记忆，状态管理
        self.current_script_name = self.all_scripts[0]          # TODO: 默认导入第一个剧本
        self.current_script:Script|None = None
        self.characters:list[Character] = []
        self.current_chartper:Charpter|None = None

        self.init_script()
    
    def init_script(self):
        script_path = self.scripts_dir / self.current_script_name
        self.current_script = self.get_script(self.current_script_name)
        self.characters = self._read_characters_from_script(script_path)
        self.update_charpter(self.current_script.intro_charpter)
    
    async def start_script(self):
        if self.current_script is None:
            logger.warning("你还没有选择一个剧本，无法开始剧本")
            return
        
        next_charpter_name = self.current_script.intro_charpter
        while next_charpter_name not in ["END", "ERROR"]:
            self.update_charpter(next_charpter_name)

            # 处理当前章节的所有事件
            while not self.events_handler.is_finished():
                await self.events_handler.process_next_event()

            # 获取下一章节的名称
            next_charpter_name = self.ends_handler.process_end()
            logger.info(f"章节结束，准备切换到: {next_charpter_name}")

        logger.info("剧本已经结束")
    
    def update_charpter(self, charpter_name: str):
        charpter_path = self.scripts_dir / self.current_script_name / "Charpters" / (charpter_name + ".yaml")
        self.current_chartper = self.charpter_manager.get_charpter(str(charpter_path))
        self.events_handler.import_events(self.current_chartper.events)
        self.ends_handler.import_end(self.current_chartper.ends)


    def get_all_scripts(self):
        self._read_all_scripts()
    
    def get_script(self, script_name:str) -> Script:
        script_path = self.scripts_dir / script_name
        return self._read_script_config(script_path)

    def _read_script_config(self, script_path):
        config = Function.read_yaml_file( script_path / "story_config.yaml" )
        if config is not None:
            return Script(config.get('script_name', 'ERROR'),config.get('description', 'ERROR'),config.get('intro_charpter', 'ERROR'))
        else:
            # TODO: 错误处理
            return Script('ERROR','ERROR','ERROR')
        
    def _read_all_scripts(self):
        
        scripts_dir = user_data_path / "game_data" / "scripts"
        logger.info("正在" + str(scripts_dir) + "中寻找剧本")

        if not scripts_dir.exists() or not scripts_dir.is_dir():
            logger.warning("剧本文件不存在")
            return
        
        for script_path in scripts_dir.iterdir():
            logger.info("找到剧本文件" + script_path.name)
            self.all_scripts.append(script_path.name)


    def _read_characters_from_script(self, script_path: Path) -> List[Character]:
        """从剧本目录读取角色 (纯 pathlib 实现)"""
        new_characters = []
        characters_dir = script_path / 'characters' # Path 对象

        if not characters_dir.exists() or not characters_dir.is_dir():
            # 使用更专业的异常和信息
            raise ScriptLoadError(f"剧本 '{script_path.name}' 中缺少 'characters' 文件夹。")

        for character_path in characters_dir.iterdir(): # 迭代 Path 对象
            # 检查是否是目录，并排除特定名称
            if not character_path.is_dir() or character_path.name == 'avatar':
                continue

            settings_path = character_path / 'settings.txt' # 使用 / 运算符连接路径
            if not settings_path.exists():
                logger.warning(f"角色目录 '{character_path.name}' 中缺少 settings.txt，已跳过。")
                continue

            try:
                settings = Function.parse_enhanced_txt(str(settings_path))
                # resource_path 现在直接是 Path 对象，如果需要字符串再转换
                
                character_id = settings.get('character_id', character_path.name)
                character = Character(
                    character_id=character_id, 
                    settings=settings, 
                    resource_path=str(character_path), 
                    memory=[{
                        "role": "system", 
                        "content": settings.get("system_prompt", "系统设定错误")
                    }]
                )
                new_characters.append(character)

            except Exception as e:
                # 记录更详细的错误日志
                logger.error(f"处理角色 '{character_path.name}' 时出错: {e}", exc_info=True)
                continue

        return new_characters