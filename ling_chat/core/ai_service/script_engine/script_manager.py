import shutil
from pathlib import Path
from typing import List, Dict, Optional

from nbformat import current_nbformat
from sqlalchemy import null

from ling_chat.utils.function import Function
from ling_chat.core.ai_service.script_engine.type import Character, Script, GameContext
from ling_chat.core.ai_service.script_engine.charpter import Charpter
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.core.ai_service.script_engine.exceptions import ScriptLoadError, ChapterLoadError, ScriptEngineError

class ScriptManager:
    def __init__(self):
        # 全局设定，确定剧本状态
        self.scripts_dir = user_data_path / "game_data" / "scripts"

        # 全部剧本管理
        self.all_scripts:list[str] = []
        self.get_all_scripts()

        self.current_script = None
        self.game_context: GameContext = GameContext()          # 创建一个空的上下文
        self.current_chartper:Charpter|None = None

        # 记忆，状态管理
        if self.all_scripts is []:
            logger.error("似乎没有scripts文件夹，请从static中导入")
            return
        
        self.current_script_name = self.all_scripts[0]          # 默认导入第一个剧本
        
        self.init_script()
    
    def init_script(self):
        script_path = self.scripts_dir / self.current_script_name
        self.current_script = self.get_script(self.current_script_name)
        characters_list:list[Character] = self._read_characters_from_script(script_path)
        if not characters_list:
            logger.error("你的剧本不存在任何角色人物")
            return
        self.game_context.characters = {
            char.character_id: char for char in characters_list
        }
    
    async def start_script(self):
        """
        剧本的主执行循环，现在变得极为清晰。
        """
        if self.current_script is None:
            logger.error("剧本不存在，请先导入剧本")
            return
        
        next_charpter_name = self.current_script.intro_charpter
        
        while next_charpter_name != "end":
            try:
                # 1. 加载章节，返回一个“可运行”的章节对象
                charpter_path = self.scripts_dir / self.current_script_name / "Charpters" / (next_charpter_name + ".yaml")
                current_charpter_obj:Charpter = self._get_charpter(charpter_path) # 一个新的辅助方法
                
                # 2. 命令章节运行，然后等待结果
                next_charpter_name = await current_charpter_obj.run()
                
            except Exception as e:
                logger.error(f"运行章节 '{next_charpter_name}' 时发生严重错误: {e}", exc_info=True)
                raise ScriptEngineError("运行章节的时候发生错误")
        
        logger.info("剧本已经结束。")

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
            raise ScriptLoadError("剧本读取出现错误,缺少 story_config.yml 配置文件")
        
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
        characters_dir = script_path / 'characters'

        if not characters_dir.exists() or not characters_dir.is_dir():
            raise ScriptLoadError(f"剧本 '{script_path.name}' 中缺少 'characters' 文件夹。")

        for character_path in characters_dir.iterdir():
            # 检查是否是目录，并排除特定名称
            if not character_path.is_dir() or character_path.name == 'avatar':
                continue

            settings_path = character_path / 'settings.txt'
            if not settings_path.exists():
                logger.warning(f"角色目录 '{character_path.name}' 中缺少 settings.txt，已跳过。")
                continue

            try:
                settings = Function.parse_enhanced_txt(str(settings_path))
                
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
                logger.error(f"处理角色 '{character_path.name}' 时出错: {e}", exc_info=True)
                continue

        return new_characters

    def _get_charpter(self, charpter_path: Path) -> Charpter:
        config = Function.read_yaml_file(charpter_path)
        if config is not None:
            return Charpter(str(charpter_path), self.game_context, config.get('events',[]), config.get('end',{}))
        else:
            raise ChapterLoadError(f"导入 {charpter_path} 剧本的时候出现问题")