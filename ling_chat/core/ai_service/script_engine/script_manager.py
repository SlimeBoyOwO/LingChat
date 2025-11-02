import shutil
from pathlib import Path
from typing import Any, List, Dict, Optional

from numpy import character
from sympy import true

from ling_chat.utils.function import Function
from ling_chat.core.ai_service.script_engine.type import Character, Player, Script, GameContext
from ling_chat.core.ai_service.script_engine.charpter import Charpter
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import user_data_path, package_root
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
        self.is_running = False

        # 记忆，状态管理
        if not self.all_scripts:
            logger.warning("剧本文件不存在，正在从static目录复制默认剧本...")
            self._copy_default_scripts()
            self.get_all_scripts()
            
        if not self.all_scripts:
            logger.error("没有可用的剧本文件")
            return
        
        self.current_script_name = self.all_scripts[0]          # 默认导入第一个剧本
        
        self.init_script()
    
    
    def _copy_default_scripts(self):
        """从static目录复制默认剧本到用户数据目录"""
        static_scripts_dir = package_root / "static" / "game_data" / "scripts"
        user_scripts_dir = user_data_path / "game_data" / "scripts"
        
        if static_scripts_dir.exists() and static_scripts_dir.is_dir():
            user_scripts_dir.mkdir(parents=True, exist_ok=True)
            for script_path in static_scripts_dir.iterdir():
                dest_path = user_scripts_dir / script_path.name
                if not dest_path.exists():
                    if script_path.is_dir():
                        shutil.copytree(script_path, dest_path)
                        logger.info(f"已复制剧本: {script_path.name}")
                    else:
                        shutil.copy2(script_path, dest_path)
                        logger.info(f"已复制文件: {script_path.name}")
        else:
            logger.error("static目录中没有找到默认剧本文件")
            
        # 复制默认角色文件夹
        static_characters_dir = package_root / "static" / "game_data" / "characters"
        user_characters_dir = user_data_path / "game_data" / "scripts" / "一只简简单单的剧情" / "characters"
        
        if static_characters_dir.exists() and static_characters_dir.is_dir():
            user_characters_dir.mkdir(parents=True, exist_ok=True)
            for character_path in static_characters_dir.iterdir():
                dest_path = user_characters_dir / character_path.name
                if not dest_path.exists():
                    if character_path.is_dir():
                        shutil.copytree(character_path, dest_path)
                        logger.info(f"已复制默认角色: {character_path.name}")
                    else:
                        shutil.copy2(character_path, dest_path)
                        logger.info(f"已复制角色文件: {character_path.name}")
        else:
            logger.error("static目录中没有找到默认角色文件")

    def init_script(self):
        if not hasattr(self, 'current_script_name') or not self.current_script_name:
            logger.error("没有可用的剧本文件，无法初始化剧本")
            return
            
        script_path = self.scripts_dir / self.current_script_name
        self.current_script = self.get_script(self.current_script_name)
        characters_list:list[Character] = self._read_characters_from_script(script_path)
        if not characters_list:
            logger.error("你的剧本不存在任何角色人物")
            return
        self.game_context.characters = {
            char.character_id: char for char in characters_list
        }
        self.game_context.player = Player(self.current_script.settings.get("user_name","无"), self.current_script.settings.get("user_subtitle","无"))
    
    async def start_script(self):
        """
        剧本的主执行循环
        """
        if self.current_script is None:
            logger.error("剧本不存在，请先导入剧本")
            return
        
        self.is_running = True
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
        
        self.is_running = False
        logger.info("剧本已经结束。")

    def get_all_scripts(self):
        self._read_all_scripts()
    
    def get_script(self, script_name:str) -> Script:
        script_path = self.scripts_dir / script_name
        return self._read_script_config(script_path)

    def get_script_characters(self, script_name) -> list[dict[str, Any]]:
        # 暂时的，仅允许读取当前导入剧本的角色
        characters = self.game_context.characters
        settings = []
        for character in characters.values():
            settings.append(character.settings)
        
        return settings

    def get_script_player(self) -> dict:
        return {
            'user_name': self.game_context.player.user_name,
            'user_subtitle': self.game_context.player.user_subtitle,
        }

    def _read_script_config(self, script_path):
        config = Function.read_yaml_file( script_path / "story_config.yaml" )
        if config is not None:
            return Script(config.get('script_name', 'ERROR'),config.get('description', 'ERROR'),config.get('intro_charpter', 'ERROR'), config.get('script_settings', {}))
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
            # 如果剧本缺少characters文件夹，尝试从静态资源复制默认角色
            static_characters_dir = package_root / "static" / "game_data" / "characters"
            if static_characters_dir.exists() and static_characters_dir.is_dir():
                characters_dir.mkdir(parents=True, exist_ok=True)
                for character_path in static_characters_dir.iterdir():
                    dest_path = characters_dir / character_path.name
                    if not dest_path.exists():
                        if character_path.is_dir():
                            shutil.copytree(character_path, dest_path)
                            logger.info(f"已为剧本 '{script_path.name}' 复制默认角色: {character_path.name}")
                        else:
                            shutil.copy2(character_path, dest_path)
                            logger.info(f"已为剧本 '{script_path.name}' 复制角色文件: {character_path.name}")
                logger.info(f"已为剧本 '{script_path.name}' 创建默认角色文件夹")
            else:
                raise ScriptLoadError(f"剧本 '{script_path.name}' 中缺少 'characters' 文件夹，且无法从静态资源复制默认角色。")

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
    

    def get_assests_dir(self) -> Path:
        return self.scripts_dir / self.current_script_name / "Assests"
    def get_avatar_dir(self, character:str) -> Path:
        return self.scripts_dir / self.current_script_name / "Characters" / character / "avatar"
    
    def is_script_running(self) -> bool:
        return self.is_running
